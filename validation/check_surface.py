# HISTORICAL HARNESS: targets the pre-decoupled architecture; port before using.
"""Targeted production WGSL surface-repair tests. Python wgpu, not native Rust/gameplay.
Run with MESA_SHADER_CACHE_DISABLE=true on memory-limited llvmpipe.
"""
from pathlib import Path
p=Path(__file__).with_name('check_compact.py');setup=p.read_text().split('sets=[{},')[0]
a=setup.index('m=d.create_shader_module(code=source)');b=setup.index("probe=r'''",a)
setup=setup[:a]+setup[b:];setup=setup.replace('m=d.create_shader_module(code=source+probe)','m=None')
a=setup.index('baseline=Path(sys.argv[1])');b=setup.index('# Build uniform',a);setup=setup[:a]+setup[b:]
exec(compile(setup,str(p),'exec'))
probe=r'''
@group(0) @binding(22) var<storage,read_write> result: array<vec4<f32>>;
@compute @workgroup_size(1)
fn surface_test() {
    let ro=frame.cam_pos;
    let rd=normalize(vec3<f32>(1.0,0.002,0.003));
    let camera=march_chunk(0u,ro,rd,16.0,false,false,false,false);
    let sun=shadow_ray(ro,rd,16.0);
    let distance=shadow_ray_t(ro,rd,16.0);
    result[0]=vec4<f32>(select(0.0,1.0,camera.hit),camera.t,select(0.0,1.0,sun),distance);
    result[1]=vec4<f32>(sun_vis_soft(ro,16.0),f32(world_block_at(vec3<f32>(1.0,8.5,8.5))),0.0,0.0);
    let sky_up=surface_sky_fill(vec3<f32>(0.0,1.0,0.0));
    let sky_down=surface_sky_fill(vec3<f32>(0.0,-1.0,0.0));
    result[2]=vec4<f32>(sky_up,1.0);
    result[3]=vec4<f32>(sky_down,1.0);
    result[4]=vec4<f32>(surface_light(vec3<f32>(0.0),vec3<f32>(0.7),0.0,vec3<f32>(0.0),0.5),1.0);
    result[5]=vec4<f32>(surface_radiance(vec3<f32>(0.8),vec3<f32>(0.0),vec3<f32>(0.0),GLOWSTONE_ID),1.0);
    let border=march_chunk(1u,ro,rd,16.0,false,false,false,false);
    result[6]=vec4<f32>(select(0.0,1.0,border.hit),border.t,0.0,0.0);
}
'''
m=d.create_shader_module(code=source+probe)
cb=d.create_buffer(size=cs*2,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_DST)
br=d.create_buffer(size=4096*4,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_DST)
grid=d.create_buffer(size=12,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_DST)
now=[]
for e in bgentries:
 e=dict(e)
 if e['binding'] in [1,4,15]:e['resource']={'buffer':{1:cb,4:br,15:grid}[e['binding']]}
 now.append(e)
bg=d.create_bind_group(layout=bgl,entries=now)
pipes={k:d.create_compute_pipeline(layout=pl,compute={'module':m,'entry_point':'surface_test','constants':k}) for k in []}
pipes=[]
for full,hq in [(True,False),(False,False),(True,True)]:
 pipes.append(d.create_compute_pipeline(layout=pl,compute={'module':m,'entry_point':'surface_test','constants':{'SPEC_FULL_MARCH':full,'SOFT_TAPS':7 if hq else 3}}))
assign(f,F,'grid_min',-1,0,0);assign(f,F,'grid_dim',3,1,1)
sd=[1,.002,.003];length=math.sqrt(sum(x*x for x in sd));assign(f,F,'sun_dir',*(x/length for x in sd))
d.queue.write_buffer(grid,0,struct.pack('<3I',0xffffffff,0,1))
def run(material,mixed=False,water=False,inside=False,empty=False,border=False):
 chunks=[]
 for index in [0,1]:
  cc=bytearray(c);assign(cc,C,'origin',index*64,0,0);assign(cc,C,'aabb_min',index*64,0,0);assign(cc,C,'aabb_max',index*64+64,64,64)
  assign(cc,C,'root',*(0,0,0,0) if empty else (0xffffffff,0xffffffff,0x80000000,0))
  assign(cc,C,'attr_flags',(0 if mixed else 1)|(2 if water else 32 if material==11 else 0));assign(cc,C,'attr_base',0 if mixed else material)
  assign(cc,C,'light_flags',1);assign(cc,C,'light_base',0xf000)
  chunks.append(cc)
 d.queue.write_buffer(cb,0,b''.join(chunks))
 entries=[]
 for l1 in range(64):
  for leaf in range(64):
   x=(l1&3)*16+(leaf&3)*4
   entries.append(0x80000000 | (1 if x>=8 else material))
 d.queue.write_buffer(br,0,struct.pack('<4096I',*entries))
 assign(f,F,'cam_pos',63.5 if border else 4.5 if inside else -.5,8.5,8.5);d.queue.write_buffer(resources[0],0,f)
 rows=[]
 for pipe in pipes:
  e=d.create_command_encoder();q=e.begin_compute_pass();q.set_pipeline(pipe);q.set_bind_group(0,bg);q.dispatch_workgroups(1);q.end();d.queue.submit([e.finish()])
  vals=struct.unpack('<512f',d.queue.read_buffer(resources[22]));rows.append([vals[i:i+4] for i in range(0,28,4)])
 return rows
for row in run(11):
 assert row[0][0]==1 and row[0][2]==0 and row[0][3]<0 and row[1][0]==1,row
 assert sum(row[2][:3])>sum(row[3][:3])*2,row
 assert max(abs(v-.7) for v in row[4][:3])<1e-6,row
 assert min(row[5][:3])>0,row
print('PASS uniform glowstone visible but no hard/soft sun shadow; emission survives darkness; direct sun not multiplied by AO; directional sky fill',flush=True)
for row in run(11,mixed=True):assert row[0][2]==1 and 8.4<row[0][3]<8.6 and row[1][0]==0,row
print('PASS stone behind mixed glowstone remains a hard/soft blocker, including FULL-root fast-path override and 8-sample quality',flush=True)
for row in run(1):assert row[0][2]==1 and row[1][0]==0,row
print('PASS opaque stone remains fully shadowed with soft shadows',flush=True)
for row in run(13,water=True):assert row[0][0]==1 and .49<row[0][1]<.51,row
print('PASS exposed water side remains visible',flush=True)
for row in run(13,water=True,mixed=True,inside=True):assert row[0][0]==1 and 3.4<row[0][1]<3.6,row
print('PASS interior water interfaces skipped without losing stone behind water',flush=True)
for row in run(13,water=True,border=True):assert row[6][0]==0,row
print('PASS water/water chunk-boundary side is not a visible wall',flush=True)
for row in run(13,water=True,empty=True):assert row[1][1]==0,row
print('PASS uniform water material does not imply occupancy in empty cells',flush=True)
print('LIMIT: synthetic fixtures, not confirmation of the reported shoreline in native gameplay or a performance measurement.')
# A real safe-donor positive control plus incompatible surface cases.
more=r'''
@compute @workgroup_size(1)
fn donor_test() {
    let rd=ray_dir(0.5,0.5);
    let t=hit_t(0u,vec3<u32>(8u),vec3<u32>(0u,3u,0u),NORMAL_PY,rd);
    let hit=frame.cam_pos+rd*t;
    result[0]=vec4<f32>(select(0.0,1.0,water_share_safe(0u,0u,NORMAL_PY,hit,t)),0.0,0.0,1.0);
}
'''
module=d.create_shader_module(code=source+probe+more)
pipe=d.create_compute_pipeline(layout=pl,compute={'module':module,'entry_point':'donor_test'})
chunks=[]
for material in [13,1]:
 cc=bytearray(c);assign(cc,C,'root',0xffffffff,0xffffffff,0x80000000,0);assign(cc,C,'attr_flags',1);assign(cc,C,'attr_base',material);chunks.append(cc)
d.queue.write_buffer(cb,0,b''.join(chunks))
assign(f,F,'res',2,2);assign(f,F,'aspect',1);assign(f,F,'tan_half_fov',.01)
assign(f,F,'cam_pos',8.5,12,8.5);assign(f,F,'cam_fwd',0,-1,0);assign(f,F,'cam_up',0,0,-1)
d.queue.write_buffer(resources[0],0,f)
def key(ci=0,n=3,y=8):return (n<<37)|(ci<<24)|(8<<16)|(y<<8)|8|(3<<14)
for label,other,expected in [('coplanar',key(),1),('sky',0,0),('side',key(n=5),0),('height',key(y=9),0),('stone',key(ci=1),0)]:
 d.queue.write_buffer(resources[9],0,struct.pack('<4Q',key(),key(),other,key()))
 enc=d.create_command_encoder();q=enc.begin_compute_pass();q.set_pipeline(pipe);q.set_bind_group(0,bg);q.dispatch_workgroups(1);q.end();d.queue.submit([enc.finish()])
 actual=struct.unpack('<f',d.queue.read_buffer(resources[22],0,4))[0]
 assert actual==expected,(label,actual,expected)
print('PASS water sharing accepts coplanar water and rejects sky, sides, different heights and stone',flush=True)
