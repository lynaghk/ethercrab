#!/usr/bin/env bash

echo -e "commit,date,text,bss,dec,bin" > sizes.csv

set -xe

for commit in $(git rev-list master)
do
    if [[ -f "examples/embassy-stm32/Cargo.toml" ]]; then
        pushd examples/embassy-stm32

        date=$(git show -s --format=%ci $commit)

        git checkout $commit

        out=$(cargo size --release | tail -n1)
        text=$(echo $out | awk '{print $1}')
        bss=$(echo $out | awk '{print $3}')
        dec=$(echo $out | awk '{print $4}')

        cargo objcopy --release -- -O binary target/size.bin
        out=$(wc -c target/demo.bin)
        bin=$(echo $out | awk '{print $1}')

        popd

        echo -e "$commit,$date,$text,$bss,$dec,$bin" >> sizes.csv
    fi
done
