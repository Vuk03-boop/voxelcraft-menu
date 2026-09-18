"""One-ray + R16F/RG16F/R8 pipeline GPU contracts, including diagnostic-only counters.
Synthetic visibility/geometry. Not native Rust compilation or target-GPU performance.
"""
from pathlib import Path
p=Path(__file__).with_name('check_compact.py');setup=p.read_text().split('sets=[{},')[0]
a=setup.index('m=d.create_shader_module(code=source)');b=setup.index("probe=r'''",a);setup=setup[:a]+setup[b:]
setup=setup.replace('m=d.create_shader_module(code=source+probe)','m=None')
a=setup.index('baseline=Path(sys.argv[1])');b=setup.index('# Build uniform',a);setup=setup[:a]+setup[b:]
exec(compile(setup,str(p),'exec'))
# Counter is injected in memory only, never saved to game WGSL.
source+='\n@group(3) @binding(6) var<storage,read_write> rays: array<atomic<u32>>;\n'
for name,index in [('shadow_ray_t',0),('shadow_ray',1)]:
 at=source.index('{',source.index('fn '+name+'('))+1
 source=source[:at]+f' atomicAdd(&rays[{index}],1u); '+source[at:]
m=d.create_shader_module(code=source)
def tex_entry(i,fmt=None):
 e={'binding':i,'visibility':wgpu.ShaderStage.COMPUTE}
 if fmt:e['storage_texture']={'access':'write-only','format':fmt,'view_dimension':'2d'}
 else:e['texture']={'sample_type':'unfilterable-float','view_dimension':'2d','multisampled':False}
 return e
count_entry={'binding':6,'visibility':wgpu.ShaderStage.COMPUTE,'buffer':{'type':'storage'}}
empty=d.create_bind_group_layout(entries=[]);eg=d.create_bind_group(layout=empty,entries=[])
layouts=[d.create_bind_group_layout(entries=es) for es in [
 [tex_entry(1,'r16float'),count_entry],
 [tex_entry(2),tex_entry(3,'rg16float'),count_entry],
 [tex_entry(2),tex_entry(4),tex_entry(5,'r8unorm'),count_entry],
 [tex_entry(0),count_entry]]]
water_layout=d.create_bind_group_layout(entries=[tex_entry(0),tex_entry(1)])
pls=[d.create_pipeline_layout(bind_group_layouts=[bgl,water_layout if i==3 else empty,empty,l]) for i,l in enumerate(layouts)]
raw=d.create_compute_pipeline(layout=pls[0],compute={'module':m,'entry_point':'primary_shadow'})
filters={}
for soft in [False,True]:
 filters[soft]=[d.create_compute_pipeline(layout=pls[i+1],compute={'module':m,'entry_point':entry,'constants':{'SPEC_SOFT_SHADOWS':soft}}) for i,entry in enumerate(['shadow_horizontal','shadow_vertical'])]
opaque=d.create_compute_pipeline(layout=pls[3],compute={'module':m,'entry_point':'resolve','constants':{'RESOLVE_LANE':0}})
flags={name:int(re.search(r'const '+name+r': u32 = (0x[0-9a-fA-F]+|\d+)u;',source)[1],0) for name in ['FLAG_SHADOWS','FLAG_HEATMAP']}
for w,h in [(8,8),(17,9)]:
 n=w*h
 usage=wgpu.TextureUsage.STORAGE_BINDING|wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_SRC|wgpu.TextureUsage.COPY_DST
 textures=[d.create_texture(size=(w,h,1),format=fmt,usage=usage) for fmt in ['r16float','rg16float','r8unorm']]
 views=[t.create_view() for t in textures]
 water_dummy=d.create_texture(size=(w,h,1),format='rgba16float',usage=wgpu.TextureUsage.TEXTURE_BINDING)
 water_group=d.create_bind_group(layout=water_layout,entries=[{'binding':i,'resource':water_dummy.create_view()} for i in [0,1]])
 counter=d.create_buffer(size=8,usage=wgpu.BufferUsage.STORAGE|wgpu.BufferUsage.COPY_SRC|wgpu.BufferUsage.COPY_DST)
 def entry(i,v):return {'binding':i,'resource':v}
 ce=entry(6,{'buffer':counter})
 groups=[d.create_bind_group(layout=l,entries=es+[ce]) for l,es in zip(layouts,[[entry(1,views[0])],[entry(2,views[0]),entry(3,views[1])],[entry(2,views[0]),entry(4,views[1]),entry(5,views[2])],[entry(0,views[2])]])]
 visbuf=buffer(bytes(n*8));chunksbuf=buffer(bytes(cs*2))
 target=d.create_texture(size=(w,h,1),format='rgba16float',usage=usage)
 bges=[]
 for e in bgentries:
  e=dict(e)
  if e['binding']==9:e['resource']={'buffer':visbuf}
  if e['binding']==1:e['resource']={'buffer':chunksbuf}
  if e['binding']==11:e['resource']=target.create_view()
  bges.append(e)
 scene=d.create_bind_group(layout=bgl,entries=bges)
 assign(f,F,'res',w,h);assign(f,F,'aspect',w/h);assign(f,F,'cam_pos',32,30,32)
 assign(f,F,'cam_fwd',0,-1,0);assign(f,F,'cam_up',0,0,-1);assign(f,F,'cam_right',1,0,0)
 assign(f,F,'sun_dir',0,1,0);assign(f,F,'tan_half_fov',.6);assign(f,F,'shadow_dist',512)
 assign(f,F,'grid_dim',1,1,1);assign(f,F,'grid_min',0,0,0);assign(f,F,'chunk_count',2)
 assign(f,F,'flags',flags['FLAG_SHADOWS']);assign(f,F,'daylight',1)
 def world(blocker):
  chunks=[]
  for ci in [0,1]:
   cc=bytearray(c);assign(cc,C,'attr_flags',1);assign(cc,C,'attr_base',1)
   assign(cc,C,'root',*(0xffffffff,0xffffffff,0x80000000,0) if ci==1 and blocker else (0,0,0,0))
   assign(cc,C,'light_flags',1);assign(cc,C,'light_base',0xf000);chunks.append(cc)
  d.queue.write_buffer(chunksbuf,0,b''.join(chunks));d.queue.write_buffer(resources[15],0,struct.pack('<I',1))
 def key(nid=3,y=16):return (nid<<37)|(16<<16)|(y<<8)|16|(3<<14)
 def upload(keys):d.queue.write_buffer(visbuf,0,struct.pack('<'+'Q'*n,*keys));d.queue.write_buffer(resources[0],0,f)
 def dispatch(enc,pipe,group):
  q=enc.begin_compute_pass();q.set_pipeline(pipe);q.set_bind_group(0,scene);q.set_bind_group(1,water_group if pipe is opaque else eg);q.set_bind_group(2,eg);q.set_bind_group(3,group);q.dispatch_workgroups((w+15)//16,(h+7)//8,1);q.end()
 def counts():return struct.unpack('<2I',d.queue.read_buffer(counter))
 def mask():return list(d.queue.read_texture({'texture':textures[2]},{'bytes_per_row':w,'rows_per_image':h},(w,h,1)))
 def filter_only(soft):
  e=d.create_command_encoder()
  for i,pipe in enumerate(filters[soft]):dispatch(e,pipe,groups[i+1])
  d.queue.submit([e.finish()]);return mask()
 for mode in ['lit','blocked','sky','backface','night','disabled','heatmap']:
  world(mode=='blocked');assign(f,F,'daylight',0 if mode=='night' else 1)
  assign(f,F,'flags',0 if mode=='disabled' else flags['FLAG_SHADOWS'] | (flags['FLAG_HEATMAP'] if mode=='heatmap' else 0))
  upload([0 if mode=='sky' else key(2 if mode=='backface' else 3)]*n)
  d.queue.write_buffer(counter,0,bytes(8));e=d.create_command_encoder();dispatch(e,raw,groups[0]);d.queue.submit([e.finish()])
  expected=n if mode in ['lit','blocked'] else 0
  assert counts()==(expected,0),(mode,counts(),expected)
  dist=struct.unpack('<'+'e'*n,d.queue.read_texture({'texture':textures[0]},{'bytes_per_row':w*2,'rows_per_image':h},(w,h,1)))
  assert all((v>0 if mode=='blocked' else v==0) for v in dist),(mode,dist)
  for soft in [False,True]:assert all(v==(0 if mode=='blocked' else 255) for v in filter_only(soft)),mode
  if mode in ['lit','blocked']:
   d.queue.write_buffer(counter,0,bytes(8));e=d.create_command_encoder();dispatch(e,opaque,groups[3]);d.queue.submit([e.finish()])
   assert counts()==(0,0),('opaque called a local tracer',counts())
 print('PASS one blocker ray per eligible pixel, none for sky/backface/night/disabled/heatmap; complete R16F/R8 writes; opaque resolve zero rays',w,h,flush=True)
 # Feed a controlled blocker-distance discontinuity to the actual bilateral passes.
 assign(f,F,'flags',flags['FLAG_SHADOWS']);assign(f,F,'daylight',1);world(False)
 def pattern(keys,dist):
  upload(keys);d.queue.write_texture({'texture':textures[0]},struct.pack('<'+'e'*n,*dist),{'bytes_per_row':w*2,'rows_per_image':h},(w,h,1));return filter_only(True)
 edge=w//2
 dist=[300.0 if x<edge else 0.0 for y in range(h) for x in range(w)]
 result=pattern([key()]*n,dist);row=result[(h//2)*w:(h//2+1)*w]
 assert 0<row[edge-1]<255 and 0<row[edge]<255,row
 # Contact hits: zero-radius reconstruction is exactly binary.
 result=pattern([key()]*n,[.001 if v else 0 for v in dist]);assert result==[0 if v else 255 for v in dist]
 # Sky and a parallel plane one block farther away must not contribute across the edge.
 for other in [0,key(y=15)]:
  result=pattern([key() if x<edge else other for y in range(h) for x in range(w)],dist)
  assert result==[0 if v else 255 for v in dist],('edge leak',other,result)
 # A 90-degree face break, both normals sun-facing, cannot borrow a shadow.
 assign(f,F,'sun_dir',.70710678,.70710678,0)
 result=pattern([key() if x<edge else key(1) for y in range(h) for x in range(w)],dist)
 assert result==[0 if v else 255 for v in dist],('corner bleed',result)
 print('PASS penumbra grows onto lit receivers; full umbra/contact preserved; sky, depth and 90-degree normal edge stops',w,h,flush=True)
print('PASS actual raw/horizontal/vertical/opaque resource-role transitions at two sizes; no performance or native-build claim.')
