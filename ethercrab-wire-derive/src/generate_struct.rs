use crate::parse_struct::StructMeta;
use proc_macro2::{Ident, Span};
use quote::quote;
use std::str::FromStr;
use syn::DeriveInput;

pub fn generate_struct_write(
    parsed: StructMeta,
    input: &DeriveInput,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let name = input.ident.clone();
    let size_bytes = parsed.width_bits.div_ceil(8);

    let fields_pack = parsed.fields.clone().into_iter().map(|field| {
        let name = field.name;
        let field_ty = field.ty;
        let byte_start = field.bytes.start;
        let bit_start = field.bit_offset;

        if field.skip {
            return quote! {};
        }

        let ty_name = field
            .ty_name
            .unwrap_or_else(|| Ident::new("UnknownTypeStopLookingAtMe", Span::call_site()));

        // Small optimisation
        if ty_name == "u8" || ty_name == "bool" {
            let mask = (2u16.pow(field.bits.len() as u32) - 1) << bit_start;
            let mask = proc_macro2::TokenStream::from_str(&format!("{:#010b}", mask)).unwrap();

            quote! {
                buf[#byte_start] |= ((self.#name as u8) << #bit_start) & #mask;
            }
        }
        // Single byte fields need merging into the other data
        else if field.bytes.len() == 1 {
            let mask = (2u16.pow(field.bits.len() as u32) - 1) << bit_start;
            let mask = proc_macro2::TokenStream::from_str(&format!("{:#010b}", mask)).unwrap();

            quote! {
                let mut field_buf = [0u8; 1];
                let res = <#field_ty as ::ethercrab_wire::EtherCrabWireWrite>::pack_to_slice_unchecked(&self.#name, &mut field_buf)[0];

                buf[#byte_start] |= (res << #bit_start) & #mask;
            }
        }
        // Assumption: multi-byte fields are byte-aligned. This should be validated during parse.
        else {
            let byte_end = field.bytes.end;

            quote! {
                <#field_ty as ::ethercrab_wire::EtherCrabWireWrite>::pack_to_slice_unchecked(&self.#name, &mut buf[#byte_start..#byte_end]);
            }
        }
    });

    let out = quote! {
        impl ::ethercrab_wire::EtherCrabWireWrite for #name {
            fn pack_to_slice_unchecked<'buf>(&self, buf: &'buf mut [u8]) -> &'buf [u8] {
                let buf = match buf.get_mut(0..#size_bytes) {
                    Some(buf) => buf,
                    None => unreachable!()
                };

                #(#fields_pack)*

                buf
            }

            fn packed_len(&self) -> usize {
                #size_bytes
            }
        }

        impl ::ethercrab_wire::EtherCrabWireWriteSized for #name {
            fn pack(&self) -> Self::Buffer {
                let mut buf = [0u8; #size_bytes];

                <Self as ::ethercrab_wire::EtherCrabWireWrite>::pack_to_slice_unchecked(self, &mut buf);

                buf
            }
        }
    };

    Ok(out)
}

pub fn generate_struct_read(
    parsed: StructMeta,
    input: &DeriveInput,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let name = input.ident.clone();
    let size_bytes = parsed.width_bits.div_ceil(8);

    let fields_unpack = parsed.fields.clone().into_iter().map(|field| {
        let ty = field.ty;
        let name = field.name;
        let byte_start = field.bytes.start;
        let bit_start = field.bit_offset;
        let ty_name = field
            .ty_name
            .unwrap_or_else(|| Ident::new("UnknownTypeStopLookingAtMe", Span::call_site()));

        if field.skip {
            return quote! {
                #name: Default::default()
            }
        }

        if field.bits.len() <= 8 {
            let mask = (2u16.pow(field.bits.len() as u32) - 1) << bit_start;
            let mask =
                proc_macro2::TokenStream::from_str(&format!("{:#010b}", mask)).unwrap();

            if ty_name == "bool" {
                quote! {
                    #name: ((buf[#byte_start] & #mask) >> #bit_start) > 0
                }
            }
            // Small optimisation
            else if ty_name == "u8" {
                quote! {
                    #name: (buf[#byte_start] & #mask) >> #bit_start
                }
            }
            // Anything else will be a struct or an enum
            else {
                quote! {
                    #name: {
                        let masked = (buf[#byte_start] & #mask) >> #bit_start;

                        <#ty as ::ethercrab_wire::EtherCrabWireRead>::unpack_from_slice(&[masked])?
                    }
                }
            }
        }
        // Assumption: multi-byte fields are byte-aligned. This must be validated during parse.
        else {
            let start_byte = field.bytes.start;
            let end_byte = field.bytes.end;

            quote! {
                #name: <#ty as ::ethercrab_wire::EtherCrabWireRead>::unpack_from_slice(&buf[#start_byte..#end_byte])?
            }
        }
    });

    let out = quote! {
        impl ::ethercrab_wire::EtherCrabWireRead for #name {
            fn unpack_from_slice(buf: &[u8]) -> Result<Self, ::ethercrab_wire::WireError> {
                let buf = buf.get(0..#size_bytes).ok_or(::ethercrab_wire::WireError::ReadBufferTooShort {
                    expected: #size_bytes,
                    got: buf.len(),
                })?;

                Ok(Self {
                    #(#fields_unpack),*
                })
            }
        }

        impl ::ethercrab_wire::EtherCrabWireSized for #name {
            const PACKED_LEN: usize = #size_bytes;

            type Buffer = [u8; #size_bytes];

            fn buffer() -> Self::Buffer {
                [0u8; #size_bytes]
            }
        }
    };

    Ok(out)
}
