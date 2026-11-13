// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
// TODO (L3) [protocol_mux]: peer publish routes shard announcements over the peer federation streams with traceparent propagation; slop: announcement unrouted, traceparent lost, shards never converge
// TODO (L3) [protocol_mux]: await-winner awaits the winning shard announcement on the mesh listener streams until the election settles; slop: no listener wait, winning shard never awaited, election never settles
// TODO (L3) [protocol_mux]: mTLS handshake framing validation over the peer federation streams across hypervisor boundaries; slop: framing unvalidated, malformed handshake bytes accepted as mTLS
