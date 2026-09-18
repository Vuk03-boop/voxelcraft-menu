"""Production glass transport over real sparse-tree fixture geometry. No native-game claim."""
from pathlib import Path
p=Path(__file__).with_name('check_compact.py');setup=p.read_text().split('sets=[{},')[0]
a=setup.index('m=d.create_shader_module(code=source)');b=setup.index("probe=r'''",a);setup=setup[:a]+setup[b:]
setup=setup.replace('m=d.create_shader_module(code=source+probe)','m=None')
a=setup.index('baseline=Path(sys.argv[1])');b=setup.index('# Build uniform',a);setup=setup[:a]+setup[b:]
exec(compile(setup,str(p),'exec'))
# Instrument only the actual production glass branches, in memory.
a=source.index('fn shade_glass(');b=source.index('fn underwater_body(',a)
part=source[a:b].replace('if id2 == WATER_ID {','if id2 == WATER_ID { result[4]=vec4<f32>(1.0);',1)
part=part.replace('if material==WATER_ID {','if material==WATER_ID { result[5]=vec4<f32>(1.0);',1)
part=part.replace('let id2 = block_at(h2.ci, h2.voxel);','let id2 = block_at(h2.ci, h2.voxel); result[6]=vec4<f32>(f32(id2),h2.t,0.0,1.0);',1)
source=source[:a]+part+source[b:]
probe=r'''
@group(0) @binding(22) var<storage,read_write> result: array<vec4<f32>>;
@compute @workgroup_size(1)
fn glass_test() {
 let ro=vec3<f32>(0.5,8.5,8.5); let rd=vec3<f32>(1.0,0.0,0.0);
 let primary=march_chunk(0u,ro,rd,frame.far,false,false,false,false);
 let transmitted=trace_world(ro,rd,max(GLASS_TRANSMIT_DIST,frame.far),false);
 let water_leg=trace_world(ro,rd,frame.far,true);
 var id=0u; if transmitted.hit { id=block_at(transmitted.ci,transmitted.voxel); }
 result[0]=vec4<f32>(primary.t,f32(id),transmitted.t,select(0.0,1.0,water_leg.hit));
 result[1]=vec4<f32>(shade_glass(0u,vec3<u32>(4u,8u,8u),0u,vec3<f32>(4.0,8.5,8.5),rd,3.5),1.0);
 // Back-side viewing exercises the glass-specific reflection ray toward the water.
 result[2]=vec4<f32>(shade_glass(0u,vec3<u32>(4u,8u,8u),0u,vec3<f32>(4.0,8.5,8.5),-rd,3.5),1.0);
}
'''
m=d.create_shader_module(code=source+probe)
cb=buffer(bytes(cs*2));ib=buffer(bytes(16));lb=buffer(struct.pack('<6I',*([0x11111111]*6)));gb=buffer(bytes(20))
out=d.create_buffer(size=2048,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC|wgpu.BufferUsage.COPY_DST)
now=[]
for e in bgentries:
 e=dict(e)
 if e['binding']==22:e['resource']={'buffer':out}
 if e['binding'] in [1,2,3,15]:e['resource']={'buffer':{1:cb,2:ib,3:lb,15:gb}[e['binding']]}
 now.append(e)
bg=d.create_bind_group(layout=bgl,entries=now)
cc=bytearray(c);assign(cc,C,'root',1,0,0,0);assign(cc,C,'dry_mask',1,0)
assign(cc,C,'aabb_min',4,8,8);assign(cc,C,'aabb_max',13,9,9)
assign(cc,C,'attr_flags',2|16);assign(cc,C,'attr_base',0);assign(cc,C,'light_flags',1);assign(cc,C,'light_base',0xf000)
c2=bytearray(c);assign(c2,C,'origin',256,0,0);assign(c2,C,'aabb_min',260,8,8);assign(c2,C,'aabb_max',261,9,9)
assign(c2,C,'root',0xffffffff,0xffffffff,0x80000000,0);assign(c2,C,'attr_flags',1);assign(c2,C,'attr_base',1);assign(c2,C,'light_flags',1);assign(c2,C,'light_base',0xf000)
assign(f,F,'grid_dim',5,1,1);assign(f,F,'chunk_count',2);assign(f,F,'far',512);assign(f,F,'cloud_scale',64)
d.queue.write_buffer(resources[0],0,f)
pipes=[d.create_compute_pipeline(layout=pl,compute={'module':m,'entry_point':'glass_test','constants':{'SPEC_GLASS_REFLECT':on}}) for on in [False,True]]
for material in [13,1]:
 d.queue.write_buffer(cb,0,cc+c2);d.queue.write_buffer(gb,0,struct.pack('<5I',0,0xffffffff,0xffffffff,0xffffffff,0xffffffff))
 d.queue.write_buffer(ib,0,struct.pack('<4I',0,(1<<9)|(1<<10)|(1<<11),0,0))
 d.queue.write_buffer(resources[4],0,struct.pack('<3I',0x8000000a,0x8000000a,0x80000000|material))
 for reflection,pipe in enumerate(pipes):
  d.queue.write_buffer(out,0,bytes(2048));e=d.create_command_encoder();q=e.begin_compute_pass();q.set_pipeline(pipe);q.set_bind_group(0,bg);q.dispatch_workgroups(1);q.end();d.queue.submit([e.finish()])
  v=struct.unpack('<512f',d.queue.read_buffer(out))
  assert abs(v[0]-3.5)<1e-5 and v[1]==material and abs(v[2]-11.5)<1e-5,v[:4]
  assert v[3]==(0 if material==13 else 1),v[:4]
  assert all(math.isfinite(x) for x in v[:28]),v[:28]
  assert min(v[4:7])>=0 and max(v[4:7])>0,v[:28]
  assert v[16]==(1 if material==13 else 0),v[:28]
  assert v[20]==(1 if material==13 and reflection else 0),v[:28]
 print('PASS primary stops at first pane; transmission skips both panes to',material,'; finite nonblack shading; reflection water branch follows its flag',flush=True)
# Same two panes, but the target is beyond the old 192-block glass limit.
d.queue.write_buffer(ib,0,struct.pack('<4I',0,(1<<9)|(1<<10),0,0))
d.queue.write_buffer(gb,0,struct.pack('<5I',0,0xffffffff,0xffffffff,0xffffffff,1))
d.queue.write_buffer(out,0,bytes(2048));e=d.create_command_encoder();q=e.begin_compute_pass();q.set_pipeline(pipes[0]);q.set_bind_group(0,bg);q.dispatch_workgroups(1);q.end();d.queue.submit([e.finish()])
v=struct.unpack('<512f',d.queue.read_buffer(out));assert v[1]==1 and v[2]>192 and v[24]==1 and v[25]>192,v[:28]
print('PASS production glass transmission reaches solid geometry beyond 192 blocks rather than substituting sky')
print('LIMIT: analytic water-through-glass remains an approximation; native scene artifacts need target retesting.')
