# sourly_cat

A CKB type script for sourly_cat

Build contracts:

``` sh
capsule build
```

Run tests:

``` sh
cd pw-lock
git submodule init
git submodule update
make install-tools
make all-via-docker
cd ..
capsule test
```
