# Contributing Quick Start

SteamHelper is a project organized within a single workspace, that plans to
offer an extensible way to communicate to Steam's Network.

Feel free to send a PR to increase the quality of this documentation.
To discuss more about the project, you can enter our Discord [here](https://discord.gg/s2YRVPy).

## Big Picture (not Steam's proprietary service)

Steam has various interconnected systems and at this page you will find how it
works so you can also contribute, or fix if some update break functionality.

### Communication

#### Protocol Buffers

Steam uses Protocol Buffer as its mechanism to serialize structured data, but on
the contrary of JSON that is human readable, protobufs are binary encoded. You
can learn more about it [here](https://developers.google.com/protocol-buffers).

Since protobufs are language neutral, on `steam-protobuf` crate we automatically
generate bindings to the rust language from the Protobufs submodule which
contains the regurlarly dumped Steam Protobufs `.proto` files.

There proto files are provided by SteamDatabase that are dumped regularly with a
tool named
[ProtobufDumper](https://github.com/SteamRE/SteamKit/tree/master/Resources/ProtobufDumper/ProtobufDumper).

#### Socket Connection

Before you start manipulating your account, you will need to establish a
connection with a Steam Content Manager Server (CM) through a socket. You can
connect to it only through a WebSocket and TcpSocket, UDP is now being
deprecated. After the connection is established, you can then login into the
network.

Steam sends messages related to your account at the socket you have connected.
These messages may or not be protobuf binary encoded messages, so we need to
check it along with its type, since any message can be trasmitted through this
stream.

We call this message we are receiving internally as `EMsg`, or Encoded Message.

Every message that needs to be sent to Steam's Network, need to be serialized,
and encrypted if that is the case. You can find helper cryptographic functions
on `steam-crypto` crate.

#### Steam Servers

### Cryptography

### SteamGuard

#### Secret Generation

#### AuthCode Generation

### Events
