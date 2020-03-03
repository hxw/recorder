# mt-recorder

A multithreaded Argon2 hasher for the bitmarkd program
with the following features:

- connects to multiple bitmarkd servers
- threads per connection are individually selectable
- individual connections have enable flag
- compatible with bitmarkd 0.12.x recorder protocol
- nosimd flavor to support older CPUs lacking these op codes
