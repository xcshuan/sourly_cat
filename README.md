# sourly_cat

A CKB type script for sourly_cat

Build contracts:

``` sh
capsule build
```

Run tests:

``` sh
git submodule init
git submodule update
cd pw-lock
make install-tools
make all-via-docker
cd ..
capsule test
```

deploy contracts:
``` sh
capsule build --release
```
release bin in build/release