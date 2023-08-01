#!/bin/bash

rm -rf target/ big-target/
TARGET_CC=x86_64-linux-musl-gcc cargo build --target x86_64-unknown-linux-musl

mkdir big-target/

cp -R target big-target/target

cd big-target

for i in $(seq 1 500); do
    ln -sf target target-$i
done
