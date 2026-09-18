"""Validate the timestamp descriptor pattern used to bracket the split family.
API check only, not a timing benchmark or execution of the Rust wrapper.
"""
import struct, wgpu
adapter=wgpu.gpu.request_adapter_sync(power_preference='low-power')
if 'timestamp-query' not in adapter.features:
    print('SKIP: timestamp-query unsupported on this adapter')
    raise SystemExit(0)
d=adapter.request_device_sync(required_features=['timestamp-query'])
qs=d.create_query_set(type='timestamp',count=16)
buf=d.create_buffer(size=256,usage=wgpu.BufferUsage.QUERY_RESOLVE|wgpu.BufferUsage.COPY_SRC)
e=d.create_command_encoder()
p=e.begin_compute_pass(timestamp_writes={'query_set':qs,'beginning_of_pass_write_index':8});p.end()
p=e.begin_compute_pass();p.end()
p=e.begin_compute_pass(timestamp_writes={'query_set':qs,'end_of_pass_write_index':9});p.end()
e.resolve_query_set(qs,0,16,buf,0);d.queue.submit([e.finish()])
t=struct.unpack('<16Q',d.queue.read_buffer(buf,0,128))
assert t[9]>=t[8]>0,t
print('PASS split-family begin/end timestamp descriptors and existing query indices 8/9')
