use crate::parse_enum::EnumStuff;
use quote::quote;
use std::str::FromStr;
use syn::DeriveInput;

pub fn generate_enum(
    parsed: EnumStuff,
    input: &DeriveInput,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let name = input.ident.clone();
    let repr_type = parsed.repr_type;

    let match_arms = parsed.variants.into_iter().map(|variant| {
        let value = proc_macro2::TokenStream::from_str(&variant.discriminant.to_string()).unwrap();
        let variant_name = variant.name;

        quote! {
            #value => { Ok(Self::#variant_name) }
        }
    });

    let out = quote! {
        // impl ::ethercrab::derive::WireFieldEnum for #name {
        //     const BYTES: usize = #repr_type::BITS as usize / 8;

        //     type Repr = #repr_type;

        //     fn unpack_to_repr(buf: &[u8]) -> Result<Self::Repr, ::ethercrab::error::Error> {
        //         let chunk = buf.get(0..Self::BYTES).ok_or(::ethercrab::error::Error::Internal)?;

        //         Ok(Self::Repr::from_le_bytes(chunk.try_into().unwrap()))
        //     }
        // }

        impl ::ethercrab::derive::WireField for #name {
            const BYTES: usize = #repr_type::BITS as usize / 8;

            fn pack_to_slice_unchecked<'buf>(&self, buf: &'buf mut [u8]) -> &'buf [u8] {
                let mut buf = &mut buf[0..Self::BYTES];

                buf.copy_from_slice(&(*self as #repr_type).to_le_bytes());

                buf
            }

            fn unpack_from_slice(buf: &[u8]) -> Result<Self, ::ethercrab::error::Error> {
                let raw = buf.get(0..Self::BYTES).map(|bytes| {
                    #repr_type::from_le_bytes(bytes.try_into().unwrap())
                }).ok_or(::ethercrab::error::Error::Internal)?;

                match raw {
                    #(#match_arms),*
                    _other => { Err(::ethercrab::error::Error::Internal) }
                }
            }
        }
    };

    Ok(out)
}
