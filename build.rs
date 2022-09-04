fn main() {
  // commit info baked into the binary
  vergen::vergen(vergen::Config::default()).unwrap();

  // generate rust bindings from protobuf IDL
  let mut config = prost_build::Config::new();
  config.bytes(&["."]);
  config
    .compile_protos(&["src/network/episub/rpc.proto"], &["src"])
    .unwrap();
}
