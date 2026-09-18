"""Read-only shader audit of the newer uploaded project. Requires Python wgpu.
The probe is appended in memory; no game source is changed.
"""
from pathlib import Path
import struct, hashlib, re, json, sys
import wgpu
ROOT=Path(sys.argv[1]) if len(sys.argv)>1 else Path(__file__).resolve().parents[1]
shader_dir=ROOT/'src/render/shaders'
source='\n'.join((shader_dir/(n+'.wgsl')).read_text() for n in ['common','tile_select','march','resolve','taa','shaft','shadow'])
adapter=wgpu.gpu.request_adapter_sync(power_preference='low-power')
print('Adapter:',dict(adapter.info),flush=True)
d=adapter.request_device_sync(required_features=['shader-int64','shader-int64-atomic-min-max','texture-adapter-specific-format-features'],required_limits=adapter.limits)
m=d.create_shader_module(code=source)
print('PASS combined WGSL validation. SHA256:',hashlib.sha256(source.encode()).hexdigest(),flush=True)
entries=['shaft_scan','tile_select','finalize','recover_select','finalize_recover','finalize_deferred','march','build_hiz','water_sec','resolve','taa']
for entry in entries:
 d.create_compute_pipeline(layout='auto',compute={'module':m,'entry_point':entry})
 print('PASS default pipeline:',entry,flush=True)
# Build non-default paths relevant to the user's reported features too.
for entry in ['march','resolve']:
 d.create_compute_pipeline(layout='auto',compute={'module':m,'entry_point':entry,'constants':{'SPEC_WIND_SWAY':True,'SPEC_WATER_LOOK':True,'SPEC_GLASS_REFLECT':True}})
 print('PASS feature pipeline:',entry,'wind/water-look/glass-reflect',flush=True)
probe=r'''
@group(0) @binding(22) var<storage,read_write> audit_out: array<vec4<f32>>;
@compute @workgroup_size(32)
fn audit_wind(@builtin(global_invocation_id) gid: vec3<u32>) {
 let i=gid.x;
 let rd=normalize(vec3<f32>(0.22,-0.10,-1.0));
 let shear=wind_shear(0.0,0.0,f32(i)*0.17);
 let q=cross_quad(vec3<u32>(0u),frame.cam_pos,rd,0u,1.0,shear);
 let reconstructed=hit_t(0u,vec3<u32>(0u),vec3<u32>(0u),q.normal,rd);
 audit_out[i]=vec4<f32>(select(0.0,1.0,q.hit),q.t,reconstructed,abs(q.t-reconstructed));
}
'''
m=d.create_shader_module(code=source+probe)
f=bytearray(352);struct.pack_into('<3f',f,0,0.1,0.85,2.0);struct.pack_into('<f',f,28,0.6);struct.pack_into('<2I',f,80,1920,1080)
chunk=bytearray(96);struct.pack_into('<f',chunk,12,1.0)
fb=d.create_buffer_with_data(data=f,usage=wgpu.BufferUsage.UNIFORM)
cb=d.create_buffer_with_data(data=chunk,usage=wgpu.BufferUsage.STORAGE)
out=d.create_buffer(size=32*16,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC)
tex=d.create_texture(size=(16,16,1),format='rgba8unorm',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_DST)
d.queue.write_texture({'texture':tex},bytes([255])*16*16*4,{'bytes_per_row':64,'rows_per_image':16},(16,16,1))
for wind in [False,True]:
 p=d.create_compute_pipeline(layout='auto',compute={'module':m,'entry_point':'audit_wind','constants':{'SPEC_WIND_SWAY':wind}})
 bg=d.create_bind_group(layout=p.get_bind_group_layout(0),entries=[{'binding':0,'resource':{'buffer':fb}},{'binding':1,'resource':{'buffer':cb}},{'binding':12,'resource':tex.create_view(dimension='2d-array')},{'binding':13,'resource':d.create_sampler()},{'binding':22,'resource':{'buffer':out}}])
 e=d.create_command_encoder();c=e.begin_compute_pass();c.set_pipeline(p);c.set_bind_group(0,bg);c.dispatch_workgroups(1);c.end();d.queue.submit([e.finish()])
 vals=struct.unpack('<128f',d.queue.read_buffer(out));rows=[vals[i*4:i*4+4] for i in range(32)]
 assert all(row[0]==1 for row in rows)
 error=max(row[3] for row in rows)
 print('Wind enabled:',wind,'hits:',len(rows),'max intersection/reconstruction disagreement in blocks:',error,'example:',max(rows,key=lambda x:x[3]),flush=True)
 if wind: assert error>0.01,'expected current-source defect did not reproduce'
 else: assert error<1e-5,'control intersection must agree'
print('CONFIRMED: current wind intersections and production hit reconstruction disagree; no fix applied.',flush=True)
