# HISTORICAL HARNESS: targets the pre-decoupled architecture; port before using.
"""GPU integration of actual resolve/shadow entry points and pass-resource roles.
Synthetic final visibility, not a full game/native build or a performance benchmark.
Requires Python wgpu. Reuses fixture setup from check_compact without its test loops.
"""
from pathlib import Path
import itertools, json, hashlib
# Reuse actual WGSL uniform-layout extraction and world/atlas fixture resources.
path=Path(__file__).with_name('check_compact.py')
setup=path.read_text()
setup=setup[:setup.index('sets=[{},')]
# Do not accumulate unrelated pipeline tests in this integration-test process.
a=setup.index('m=d.create_shader_module(code=source)')
b=setup.index("probe=r'''",a)
setup=setup[:a]+setup[b:]
setup=setup.replace('m=d.create_shader_module(code=source+probe)','m=None')
a=setup.index('baseline=Path(sys.argv[1])');b=setup.index('# Build uniform',a)
setup=setup[:a]+setup[b:]
exec(compile(setup,str(path),'exec'))
# Remove the diagnostic probe from the module for all production pipeline tests.
ownership=len(sys.argv)>1 and sys.argv[1]=='ownership'
render_source=source
if ownership:
    # Diagnostic-only writes: ownership and local shadow calls. Never saved to WGSL.
    render_source+='\n@group(3) @binding(1) var<storage,read_write> owner_count: array<atomic<u32>>;\n@group(3) @binding(2) var<storage,read_write> ray_count: array<atomic<u32>>;\n'
    render_source=render_source.replace('textureStore(out_tex,','atomicAdd(&owner_count[pix],1u); textureStore(out_tex,',2)
    for name in ['shadow_ray','shadow_ray_t']:
        at=render_source.index('{',render_source.index('fn '+name+'('))+1
        render_source=render_source[:at]+' atomicAdd(&ray_count[0],1u); '+render_source[at:]
module=d.create_shader_module(code=render_source)

def flag(name):
    return int(re.search(r'const '+name+r': u32 = (0x[0-9A-Fa-f]+|\d+)u;',source)[1],0)

def make_layout(entries):return d.create_bind_group_layout(entries=entries)
empty=make_layout([]);empty_group=d.create_bind_group(layout=empty,entries=[])
read_layout=make_layout([{'binding':i,'visibility':wgpu.ShaderStage.COMPUTE,'texture':{'sample_type':'float','view_dimension':'2d','multisampled':False}} for i in [0,1]])
store_layout=make_layout([{'binding':i,'visibility':wgpu.ShaderStage.COMPUTE,'storage_texture':{'access':'write-only','format':'rgba16float','view_dimension':'2d'}} for i in [0,1]])
shadow_layout=make_layout([{'binding':i,'visibility':wgpu.ShaderStage.COMPUTE,'buffer':{'type':'storage'}} for i in (range(3) if ownership else range(1))])
resolve_layout=d.create_pipeline_layout(bind_group_layouts=[bgl,read_layout,empty,shadow_layout])
water_layout=d.create_pipeline_layout(bind_group_layouts=[bgl,empty,store_layout])

def pipeline(name,constants,lane=0):
    print("Compiling",name,lane,constants,flush=True)
    return d.create_compute_pipeline(layout=water_layout if name=='water_sec' else resolve_layout,
        compute={'module':module,'entry_point':name,'constants':dict(constants,RESOLVE_LANE=lane)})

# Two feature combinations exercise the new passes alone and alongside the older arms.
import sys
profiles={
    'basic': {'SPEC_WATER_SEC':False},
    'ownership': {'SPEC_WATER_SEC':False},
    'shared': {'SPEC_WATER_SEC':True,'SPEC_COMPACT_SHADE_HIT':True},
    'soft': {'SPEC_WATER_SEC':True,'SPEC_SOFT_SHADOWS':True},
    'combined': {'SPEC_WATER_SEC':True,'SPEC_COMPACT_SHADE_HIT':True,'SPEC_SOFT_SHADOWS':True},
    'effects': {'SPEC_WATER_SEC':True,'SPEC_GLASS_REFLECT':True,'SPEC_WATER_LOOK':True,'SPEC_WIND_SWAY':True},
}
profile=sys.argv[1] if len(sys.argv)>1 else 'basic'
feature_sets=[profiles[profile]]
cases=0;max_error=0.0
for features in feature_sets:
    # Fixtures contain no emitter blocks. Exercise the production no-emitter specialization.
    features=dict(features,SPEC_EMITTER_GATHER=False)
    d.queue.write_buffer(resources[4],0,struct.pack('<4096I',*[0x80000000 | ((8+(i%8))<<12) for i in range(4096)]))
    variants={}
    choice=sys.argv[2] if len(sys.argv)>2 else 'shadow'
    pairs={'control':[(False,False)],'glass':[(True,False)],'shadow':[(False,True)],'both':[(True,True)]}[choice]
    reference_path=R/('.surface-reference-'+profile+'.json')
    stamp=hashlib.sha256((source+Path(__file__).read_text()+json.dumps(features,sort_keys=True)).encode()).hexdigest()
    records={'stamp':stamp,'adapter':dict(adapter.info),'images':{}}
    if choice!='control':
        records=json.loads(reference_path.read_text())
        assert records['stamp']==stamp and records['adapter']==dict(adapter.info),'stale or different-adapter control; rerun control'

    for glass,shadow in pairs:
        constants=dict(features,SPEC_ISOLATE_GLASS=glass,SPEC_SHADOW_PASS=shadow)
        variants[glass,shadow]=(pipeline('resolve',constants),
            pipeline('primary_shadow',constants) if shadow else None,
            pipeline('resolve',constants,1) if glass or shadow else None,
            pipeline('resolve',constants,2) if shadow else None,
            pipeline('water_sec',constants) if features['SPEC_WATER_SEC'] else None)
    for w,h in [(8,8),(17,9)]:
        pixels=w*h
        target=d.create_texture(size=(w,h,1),format='rgba16float',usage=wgpu.TextureUsage.STORAGE_BINDING|wgpu.TextureUsage.COPY_SRC|wgpu.TextureUsage.COPY_DST)
        visbuf=buffer(bytes(pixels*8));dbgbuf=buffer(bytes(pixels*4))
        shadowbuf=d.create_buffer(size=pixels*4,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC|wgpu.BufferUsage.COPY_DST)
        shadow_entries=[{'binding':0,'resource':{'buffer':shadowbuf}}]
        if ownership:
            ownerbuf=d.create_buffer(size=pixels*4,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC|wgpu.BufferUsage.COPY_DST)
            raybuf=d.create_buffer(size=4,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC|wgpu.BufferUsage.COPY_DST)
            shadow_entries += [{'binding':1,'resource':{'buffer':ownerbuf}},{'binding':2,'resource':{'buffer':raybuf}}]
        shadowgroup=d.create_bind_group(layout=shadow_layout,entries=shadow_entries)
        pair=[d.create_texture(size=((w+1)//2,(h+1)//2,1),format='rgba16float',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.STORAGE_BINDING) for _ in range(2)]
        readgroup=d.create_bind_group(layout=read_layout,entries=[{'binding':i,'resource':pair[i].create_view()} for i in [0,1]])
        storegroup=d.create_bind_group(layout=store_layout,entries=[{'binding':i,'resource':pair[i].create_view()} for i in [0,1]])
        chunks_data=bytearray(cs*4)
        for ci,material in enumerate([3,10,13,18]):
            chunk=bytearray(c)
            assign(chunk,C,'attr_flags',1);assign(chunk,C,'attr_base',material)
            chunks_data[ci*cs:(ci+1)*cs]=chunk
        chunksbuf=buffer(chunks_data)
        entries_now=[]
        for entry in bgentries:
            e=dict(entry);i=e['binding']
            if i==1:e['resource']={'buffer':chunksbuf}
            if i==9:e['resource']={'buffer':visbuf}
            if i==14:e['resource']={'buffer':dbgbuf}
            if i==11:e['resource']=target.create_view()
            entries_now.append(e)
        scene_group=d.create_bind_group(layout=bgl,entries=entries_now)
        assign(f,F,'res',w,h);assign(f,F,'aspect',w/h);assign(f,F,'tan_half_fov',.22)
        assign(f,F,'cam_fwd',.12,-.08,-1);assign(f,F,'cloud_scale',64)
        assign(f,F,'shadow_dist',8);assign(f,F,'chunk_count',4);assign(f,F,'cloud_shadow',0)
        assign(f,F,'fog_density',.002);assign(f,F,'fog_falloff',.01);assign(f,F,'fog_height',20)
        assign(f,F,'wave_amp',.1);assign(f,F,'wave_speed',1);assign(f,F,'wave_scale',16)
        def dispatch(e,pipe,groups,water=False):
            p=e.begin_compute_pass();p.set_pipeline(pipe);p.set_bind_group(0,scene_group)
            if water:p.set_bind_group(1,empty_group);p.set_bind_group(2,storegroup)
            else:p.set_bind_group(1,readgroup);p.set_bind_group(2,empty_group);p.set_bind_group(3,shadowgroup)
            p.dispatch_workgroups(*groups);p.end()
        for mode in ['mixed','sky','glass','underwater','heatmap','no-shadows','night','occluded','water-top']:
            assign(f,F,'cam_fwd',.12,-.8 if mode=='water-top' else -.08,-1)
            assign(f,F,'tan_half_fov',.02 if mode=='water-top' else .22)
            flags=flag('FLAG_SHADOWS')|flag('FLAG_AO')|flag('FLAG_TINT')|flag('FLAG_WATER_REFLECT')|flag('FLAG_WATER_REFRACT')
            if mode=='underwater':flags|=flag('FLAG_UNDERWATER')
            if mode=='heatmap':flags|=flag('FLAG_HEATMAP')
            if mode=='no-shadows':flags &= ~flag('FLAG_SHADOWS')
            assign(f,F,'flags',flags);assign(f,F,'cam_pos',24,10 if mode=='underwater' else 24,80)
            assign(f,F,'daylight',0 if mode=='night' else .85);d.queue.write_buffer(resources[0],0,f)
            for ci in range(4):
                chunk=bytearray(chunks_data[ci*cs:(ci+1)*cs])
                assign(chunk,C,'root',*(0xffffffff,0xffffffff,0x80000000,0) if mode=='occluded' and ci==0 else (0,0,0,0))
                chunks_data[ci*cs:(ci+1)*cs]=chunk
            d.queue.write_buffer(chunksbuf,0,chunks_data)
            keys=[]
            for i in range(pixels):
                ci=2 if mode=='water-top' else 1 if mode=='glass' else i%4
                normal=3 if mode=='water-top' else 6+(i%2) if ci==3 else 5
                # Same packed visibility format used by production resolve.
                low=(16<<16)|(16<<8)|31
                if mode=='water-top':low |= 3<<14
                keys.append(0 if mode=='sky' or (mode=='mixed' and i%11==0) else (normal<<37)|(ci<<24)|low)
            d.queue.write_buffer(visbuf,0,struct.pack('<'+'Q'*pixels,*keys))
            key_image=f"{w}x{h}/{mode}"
            reference=None if choice=='control' else records['images'][key_image]
            for variant,pipes in variants.items():
                main,shadow,glass,water,legs=pipes
                # Sentinel proves no sky/material/edge pixel is left unwritten.
                d.queue.write_texture({'texture':target},struct.pack('<4e',-7,-7,-7,-7)*pixels,{'bytes_per_row':w*8,'rows_per_image':h},(w,h,1))
                d.queue.write_buffer(shadowbuf,0,struct.pack('<'+'f'*pixels,*([-7.0]*pixels)))
                if ownership:
                    d.queue.write_buffer(ownerbuf,0,bytes(pixels*4));d.queue.write_buffer(raybuf,0,bytes(4))
                enc=d.create_command_encoder()
                if shadow:dispatch(enc,shadow,((w+15)//16,(h+7)//8,1))
                if legs:dispatch(enc,legs,(((w+1)//2+7)//8,((h+1)//2+7)//8,1),True)
                dispatch(enc,main,((w+15)//16,(h+7)//8,1))
                if glass:dispatch(enc,glass,((w+15)//16,(h+7)//8,1))
                if water:dispatch(enc,water,((w+15)//16,(h+7)//8,1))
                d.queue.submit([enc.finish()])
                rgba=struct.unpack('<'+'e'*(pixels*4),d.queue.read_texture({'texture':target},{'bytes_per_row':w*8,'rows_per_image':h},(w,h,1)))
                assert all(math.isfinite(v) for v in rgba),(features,mode,variant,'nonfinite')
                assert all(rgba[i*4+3]==1 for i in range(pixels)),(mode,variant,'unwritten pixels')
                if reference is None:
                    reference=rgba;records['images'][key_image]=rgba
                else:
                    error=max(abs(a-b) for a,b in zip(reference,rgba));max_error=max(max_error,error)
                    assert error<=1e-3,(features,(w,h),mode,variant,error)
                    cases+=1
                if shadow:
                    visibility=struct.unpack('<'+'f'*pixels,d.queue.read_buffer(shadowbuf))
                    assert all(0<=v<=1 for v in visibility),(mode,'shadow coverage')
                    if mode=='occluded':assert min(visibility)<1,'occluded control must actually shadow something'
                if ownership:
                    owners=struct.unpack('<'+'I'*pixels,d.queue.read_buffer(ownerbuf))
                    assert all(v==1 for v in owners),(variant,mode,'duplicate or missing material writer')
                    d.queue.write_buffer(raybuf,0,bytes(4))
                    isolated=d.create_command_encoder();dispatch(isolated,main,((w+15)//16,(h+7)//8,1));d.queue.submit([isolated.finish()])
                    calls=struct.unpack('<I',d.queue.read_buffer(raybuf))[0]
                    if variant[1]:assert calls==0,('shadow tracer still reached from opaque resolve',calls)
                    elif mode=='mixed':assert calls>0,'positive control did not reach inline shadow tracing'
        print('PASS split family resource/output comparison:',features,(w,h),flush=True)
if choice=='control':
    reference_path.write_text(json.dumps(records))
    print('PASS recorded 18 control images for independent-process comparison',flush=True)
if ownership:print('PASS exactly one HDR writer per pixel; split opaque zero-tracer assertions / unsplit positive controls checked for selected architecture')
print(f'PASS {cases} split-vs-unsplit image comparisons; max HDR channel error {max_error}',flush=True)
print('Includes odd-size resize/rebind, shared/full water, glass, foliage, sky, underwater, heatmap, disabled shadows and night. No Rust build or performance claim.')
