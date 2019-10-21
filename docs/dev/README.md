# Contributing Quick Start

SteamHelper is a project organized within a single workspace, that plans to offer an extensible way to communicate
to Steam's Network.

## Big Picture (not Steam's proprietary service)

### Communication

Steam uses Protocol Buffer as its mechanism to serialize structured data, but on
the contrary of JSON that is human readable, protobufs are binary encoded. You
can learn more about it (here)[https://developers.google.com/protocol-buffers].

Since protobufs are language neutral, on `steam-protobuf` crate we automatically
generate bindings to the rust language from the Protobufs submodule which
contains the regurlarly dumped Steam Protobufs `.proto` files.

There proto files are provided by SteamDatabase that are dumped regularly with a
tool named
(ProtobufDumper)[https://github.com/SteamRE/SteamKit/tree/master/Resources/ProtobufDumper/ProtobufDumper]

Every message that needs to be sent to Steam's Network, need to be serialized,
and encrypted if that is the case. You can find helper cryptographic functions
on `steam-crypto` crate.

### Steam Servers
