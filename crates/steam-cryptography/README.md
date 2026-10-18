# Steam-Cryptography

Cryptography used by the Steam CM protocol, the one the Steam client speaks.
It is used by steam-client-lib.

When a client connects, Steam sends a random nonce. The client generates a
session key, encrypts it with the Steam universe's RSA public key, and sends it
back. From then on every packet is encrypted with AES-256 using that session
key, with an IV that carries an HMAC of the message.

This is a port of SteamKit's
[CryptoHelper](https://github.com/SteamRE/SteamKit/blob/master/SteamKit2/SteamKit2/Util/CryptoHelper.cs)
and [node-steam-crypto](https://github.com/DoctorMcKay/node-steam-crypto),
built on top of [RustCrypto](https://github.com/RustCrypto).

## Implemented
* Session key generation and RSA encryption for the channel handshake;
* AES-256 encryption and decryption of packets, with HMAC IVs;
* Decryption in place, so reading packets doesn't allocate;
* Signature verification and CRC hashes;

Must keep 100% test coverage, so send those tests along with your PRs!
