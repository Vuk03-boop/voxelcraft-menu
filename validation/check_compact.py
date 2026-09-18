"""Production-helper GPU A/B, not a whole-scene capture or register measurement.
Requires Python wgpu. All fixture storage is local; no game files are modified.
"""
from pathlib import Path
import re, struct, math, sys
import wgpu
R=Path(__file__).resolve().parents[1];S=R/'src/render/shaders'
source='\n'.join((S/(n+'.wgsl')).read_text() for n in ['common','tile_select','march','resolve','taa','shaft','shadow'])
adapter=wgpu.gpu.request_adapter_sync(power_preference='low-power')
d=adapter.request_device_sync(required_features=['shader-int64','shader-int64-atomic-min-max','texture-adapter-specific-format-features'],required_limits=adapter.limits)
print('Adapter:',dict(adapter.info),flush=True)
m=d.create_shader_module(code=source)
# Each experimental switch individually, plus the combination. These are shader
# compile checks, not an endorsement of the pre-existing rendering experiments.
# Production entry points are checked separately to bound compiler memory.
probe=r'''
@group(0) @binding(22) var<storage, read_write> test_color: array<vec4<f32>>;
@compute @workgroup_size(32)
fn check_compact(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i=gid.x;
    let normal_id=i%8u;
    let cell=array<u32,4>(0u,15u,31u,63u)[(i/8u)%4u];
    let v=vec3<u32>(cell,16u,cell);
    let hit=vec3<f32>(v)+vec3<f32>(0.13+f32(i%7u)*0.11,0.7,0.23);
    let rd=normalize(vec3<f32>(0.22,select(-0.3,0.3,i%2u==0u),-1.0));
    let id=array<u32,6>(GRASS_ID,SAND_ID,LEAVES_ID,PINE_LEAVES_ID,TALL_GRASS_ID,MEADOW_ID)[(i/4u)%6u];
    let color=shade_hit(0u,v,id,normal_id,hit,rd,1.0+f32(i)*3.1,8.0,i%3u!=0u,i%2u==0u);
    test_color[i]=vec4<f32>(color,1.0);
}
'''
m=d.create_shader_module(code=source+probe)
baseline=Path(sys.argv[1]) if len(sys.argv)>1 else R/'.patch-base'
old_module=None
print('Surface repair changes intended lighting; historical original-baseline image comparison is retired.',flush=True)
# Build uniform offsets from the actual WGSL definition, not a stale mirrored list.
def layout(name):
    body=re.search(r'struct '+name+r'\s*\{(.*?)\n\};',source,re.S)[1]
    body=re.sub(r'//[^\n]*','',body);offset=0;fields={}
    for key,typ in re.findall(r'(\w+)\s*:\s*([^,]+(?:<[^>]+>)?)\s*,',body):
        typ=typ.strip();base='I' if 'u32' in typ else 'i' if 'i32' in typ else 'f'
        n=16 if 'mat4' in typ else int(re.search(r'vec(\d)',typ)[1]) if 'vec' in typ else 1
        align=16 if n>=3 else n*4;offset=(offset+align-1)//align*align
        fields[key]=(offset,'<'+base*n);offset+=n*4
    return fields,(offset+15)//16*16
F,fs=layout('Frame');C,cs=layout('Chunk');assert fs==352 and cs==96,(fs,cs)
f=bytearray(fs);c=bytearray(cs)
def assign(buf,fields,key,*values):struct.pack_into(fields[key][1],buf,fields[key][0],*values)
for key,vals in {'cam_pos':(8,24,70),'cam_fwd':(0,0,-1),'cam_up':(0,1,0),'cam_right':(1,0,0),
    'tan_half_fov':(.6,),'res':(1280,720),'tiles':(160,90),'sun_dir':(.3,.8,.4),'daylight':(.85,),
    'far':(256,),'chunk_count':(1,),'grid_dim':(1,1,1),'ambient':(.08,),'sea_level':(20,),
    'tint_strength':(.7,),'tint_freq_t':(.01,),'tint_freq_h':(.02,),'tint_seed':(42,),
    'wave_scale':(16,),'shaft_texel':(4,),'water_absorb':(1,)}.items():assign(f,F,key,*vals)
assign(c,C,'voxel_size',1);assign(c,C,'aabb_max',64,64,64)
assign(c,C,'light_flags',0);assign(c,C,'light_base',0)
# Vary packed RGB block light and skylight by 4^3 light brick; exercise gather math.
lights=[0x80000000 | ((8+(i%8))<<12) | ((i%16)<<8) | (((i//3)%16)<<4) | ((i//7)%16) for i in range(4096)]
def buffer(data,usage=wgpu.BufferUsage.STORAGE):return d.create_buffer_with_data(data=data,usage=usage|wgpu.BufferUsage.COPY_DST)
resources={0:buffer(f,wgpu.BufferUsage.UNIFORM),1:buffer(c),2:buffer(bytes(32)),3:buffer(bytes(16)),4:buffer(struct.pack('<4096I',*lights)),
    5:buffer(struct.pack('<256I',*[(i%4)|((i%3)<<16) for i in range(256)])),15:buffer(struct.pack('<I',0)),19:buffer(bytes(4096)),22:d.create_buffer(size=128*16,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC)}
for i in [6,7,8,9,10,14]:resources[i]=buffer(bytes(256))
tex=d.create_texture(size=(16,16,4),mip_level_count=4,format='rgba8unorm',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_DST)
for mip in range(4):
    size=16>>mip;data=bytes(v for z in range(4) for y in range(size) for x in range(size) for v in [(x*17+z*41)%256,(y*17+mip*53)%256,80+z*30,64+(x+y)%4*48])
    d.queue.write_texture({'texture':tex,'mip_level':mip},data,{'bytes_per_row':size*4,'rows_per_image':size},(size,size,4))
resources[12]=tex.create_view(dimension='2d-array')
pt=d.create_texture(size=(2,2,2),dimension='3d',format='rgba16float',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_DST)
d.queue.write_texture({'texture':pt},struct.pack('<4e',.9,1.0,1.1,.8)*8,{'bytes_per_row':16,'rows_per_image':2},(2,2,2));resources[20]=pt.create_view()
for i in [13,18,21]:resources[i]=d.create_sampler(mag_filter='linear',min_filter='linear',mipmap_filter='linear')
for i in [11,16,17]:
    t=d.create_texture(size=(4,4,1),format='rgba16float',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.STORAGE_BINDING)
    resources[i]=t.create_view()
entries=[];bgentries=[]
for i in range(23):
    e={'binding':i,'visibility':wgpu.ShaderStage.COMPUTE}
    if i in [13,18,21]:e['sampler']={'type':'filtering'};res=resources[i]
    elif i==11:e['storage_texture']={'access':'write-only','format':'rgba16float','view_dimension':'2d'};res=resources[i]
    elif i in [12,16,17,20]:e['texture']={'sample_type':'float','view_dimension':'2d-array' if i==12 else '3d' if i==20 else '2d','multisampled':False};res=resources[i]
    else:e['buffer']={'type':'uniform' if i==0 else 'read-only-storage' if i in [1,2,3,4,5,15] else 'storage'};res={'buffer':resources[i]}
    entries.append(e);bgentries.append({'binding':i,'resource':res})
bgl=d.create_bind_group_layout(entries=entries);pl=d.create_pipeline_layout(bind_group_layouts=[bgl]);bg=d.create_bind_group(layout=bgl,entries=bgentries)
sets=[{}, {'SPEC_FOLIAGE_RICH':True,'SPEC_CANOPY_RELIEF':True,'SPEC_CAUSTICS':True},
      {'SPEC_PROBE_AMBIENT':True}, {'SPEC_LIGHT_RGB':False,'SPEC_SKY_SPECULAR':False}, {'SPEC_SOFT_SHADOWS':True}]
# Avoid recomputing compiler pipelines for each runtime fixture.
count=0;max_error=0.0
for features in sets:
    pipes=[d.create_compute_pipeline(layout=pl,compute={'module':m,'entry_point':'check_compact','constants':dict(features,SPEC_COMPACT_SHADE_HIT=arm)}) for arm in [False,True]]
    if old_module is not None:
        pipes.append(d.create_compute_pipeline(layout=pl,compute={'module':old_module,'entry_point':'check_compact','constants':features}))
    for full in [False,True]:
        assign(c,C,'root',*(0xffffffff,0xffffffff,0x80000000,0) if full else (0,0,0,0));d.queue.write_buffer(resources[1],0,c)
        for flags in [0,4|8|128|16384|32768]:
            assign(f,F,'flags',flags);d.queue.write_buffer(resources[0],0,f);outputs=[]
            for pipe in pipes:
                e=d.create_command_encoder();p=e.begin_compute_pass();p.set_pipeline(pipe);p.set_bind_group(0,bg);p.dispatch_workgroups(4);p.end();d.queue.submit([e.finish()])
                outputs.append(struct.unpack('<512f',d.queue.read_buffer(resources[22])))
            assert all(math.isfinite(x) for v in outputs for x in v),'nonfinite fixture'
            err=max(abs(a-b) for other in outputs[1:] for a,b in zip(outputs[0],other));assert err<=1e-6,(features,full,flags,err)
            max_error=max(max_error,err);count+=128
    print('PASS helper A/B:',features or 'default specializations',flush=True)
print(f'PASS {count} shader fixtures; maximum channel error {max_error}',flush=True)
print('Comparisons: legacy vs compact' + (' AND legacy vs original baseline' if old_module is not None else '; original-baseline comparison retired for intentional visual changes'))
# Positive control: prove the selector actually reaches the compact return. This
# mutation exists only in memory and never enters the game shader or saved patch.
needle='return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);'
assert source.count(needle)>=1
mut=d.create_shader_module(code=source.replace(needle,'return surface_radiance(albedo, light_rgb * face_scale, spec_value, id) + vec3<f32>(0.125, 0.0, 0.0);',1)+probe)
positive=[]
for arm in [False,True]:
    pipe=d.create_compute_pipeline(layout=pl,compute={'module':mut,'entry_point':'check_compact','constants':{'SPEC_COMPACT_SHADE_HIT':arm}})
    e=d.create_command_encoder();p=e.begin_compute_pass();p.set_pipeline(pipe);p.set_bind_group(0,bg);p.dispatch_workgroups(4);p.end();d.queue.submit([e.finish()])
    positive.append(struct.unpack('<512f',d.queue.read_buffer(resources[22])))
assert all(abs((positive[1][i*4]-positive[0][i*4])-0.125)<1e-5 for i in range(128))
print('PASS positive control: compact-only in-memory marker reached all 128 samples; selector is active')
print('Not a whole-scene visual verdict, Rust build, GPU register count or FPS benchmark.')
