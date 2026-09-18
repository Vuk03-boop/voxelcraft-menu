"""Source/syntax and GPU presentation contracts, NOT a native Rust build.
Usage (with dependencies installed): python3 validation/check_menu.py
Dependencies: tree-sitter, tree-sitter-rust, wgpu (tested with Python wgpu 0.32).
"""
from pathlib import Path
import hashlib, struct, re
from tree_sitter import Language, Parser
import tree_sitter_rust
import wgpu
ROOT=Path(__file__).resolve().parents[1]
parser=Parser(Language(tree_sitter_rust.language()))
files=[p for p in ROOT.rglob('*.rs') if not any(part.startswith('.') for part in p.relative_to(ROOT).parts)]
for path in files:
    assert not parser.parse(path.read_bytes()).root_node.has_error, path
print(f'PASS Rust syntax: {len(files)} files (not type checking)')
app=(ROOT/'src/app.rs').read_text()
frame=app[app.index('    fn frame(&mut self)'):]
assert frame.index('self.menu.screen != Screen::Playing') < frame.index('self.stats.tick()') < frame.index('self.player')
assert 'start.elapsed()' not in app and 'time: self.animation_time' in app
assert 'scale_index' not in app and 'self.taa' not in app
assert len(re.findall(r'\bRenderer::new\(',app))==1
resumed=app[app.index('    fn resumed('):app.index('    fn device_event(')]
assert not re.search(r'\bRenderer::new',resumed)
for path in ['src/headless.rs','src/config.rs','src/scene.rs']:
    assert 'settings::load' not in (ROOT/path).read_text()
print('PASS source guards: deferred renderer, pause before simulation, frozen clock, unified scale/TAA, headless isolation')
# All preference CLI aliases are real parser arms, not guessed option names.
prefs=(ROOT/'src/settings.rs').read_text()
overlay=prefs[prefs.index('pub fn overlay_cli'):prefs.index('pub fn encode')]
config=(ROOT/'src/config.rs').read_text()
for flag in re.findall(r'"(--[\w-]+)"',overlay):
    assert f'"{flag}" =>' in config,flag
print('PASS all preference CLI aliases match the parser')
assert 'self.session_started && plan.experiments' in app
assert 'self.menu.experiments_locked = true' in app
print('PASS title-only specialization guards')
# Baseline checks run when the preserved extraction is beside the working copy.
baseline=ROOT/'.patch-base'
if baseline.exists():
    untouched=['Cargo.toml','src/headless.rs']
    untouched += [str(p.relative_to(ROOT)) for p in (ROOT/'src/render/shaders').glob('*.wgsl') if p.name not in ['common.wgsl','resolve.wgsl','march.wgsl']]
    for name in untouched:
        assert (ROOT/name).read_bytes()==(baseline/name).read_bytes(),name
    print(f'PASS {len(untouched)} untouched baseline manifest/headless/shader files byte-identical')

resolve=(ROOT/'src/render/shaders/resolve.wgsl').read_text()
assert resolve.count('let sky_tint = surface_sky_fill(n);')==4
assert resolve.count('visibility = lit;')==4
assert resolve.count('surface_radiance(albedo,')==4
print('PASS all four lighting schedules share the authorized surface repair composition')

adapter=wgpu.gpu.request_adapter_sync(power_preference='low-power')
d=adapter.request_device_sync()
print('GPU adapter:',dict(adapter.info))
shaders=ROOT/'src/render/shaders'
ui=d.create_shader_module(code=(shaders/'ui.wgsl').read_text())
blit=d.create_shader_module(code=(shaders/'blit.wgsl').read_text())
target=d.create_texture(size=(4,4,1),format='rgba8unorm',usage=wgpu.TextureUsage.RENDER_ATTACHMENT|wgpu.TextureUsage.COPY_SRC)
blend={'color':{'src_factor':'src-alpha','dst_factor':'one-minus-src-alpha','operation':'add'},
       'alpha':{'src_factor':'one','dst_factor':'one-minus-src-alpha','operation':'add'}}
uip=d.create_render_pipeline(layout='auto',vertex={'module':ui,'entry_point':'vs','buffers':[{'array_stride':32,'step_mode':'vertex','attributes':[
    {'format':'float32x2','offset':0,'shader_location':0},{'format':'float32x2','offset':8,'shader_location':1},{'format':'float32x4','offset':16,'shader_location':2}]}]},
    fragment={'module':ui,'entry_point':'fs','targets':[{'format':'rgba8unorm','blend':blend}]})
bp=d.create_render_pipeline(layout='auto',vertex={'module':blit,'entry_point':'vs','buffers':[]},
    fragment={'module':blit,'entry_point':'fs','targets':[{'format':'rgba8unorm'}]})
print('PASS production UI and blit render pipelines')
font=d.create_texture(size=(1,1,1),format='r8unorm',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_DST)
d.queue.write_texture({'texture':font},b'\xff',{'bytes_per_row':1,'rows_per_image':1},(1,1,1))
u=d.create_buffer_with_data(data=struct.pack('<4f',4,4,0,0),usage=wgpu.BufferUsage.UNIFORM)
ubg=d.create_bind_group(layout=uip.get_bind_group_layout(0),entries=[{'binding':0,'resource':{'buffer':u}},{'binding':1,'resource':font.create_view()},{'binding':2,'resource':d.create_sampler()}])
verts=[]
for x,y in [(0,0),(4,0),(4,4),(0,0),(4,4),(0,4)]:verts.extend([x,y,-1,-1,0.2,0.4,0.6,1])
vb=d.create_buffer_with_data(data=struct.pack('<48f',*verts),usage=wgpu.BufferUsage.VERTEX)
def draw(pipeline,bg,vertices=None):
    e=d.create_command_encoder();p=e.begin_render_pass(color_attachments=[{'view':target.create_view(),'resolve_target':None,'load_op':'clear','store_op':'store','clear_value':(0,0,0,1)}])
    p.set_pipeline(pipeline);p.set_bind_group(0,bg)
    if vertices is not None:p.set_vertex_buffer(0,vertices)
    p.draw(6 if vertices is not None else 3);p.end();d.queue.submit([e.finish()])
    return bytes(d.queue.read_texture({'texture':target},{'bytes_per_row':16,'rows_per_image':4},(4,4,1)))
assert draw(uip,ubg,vb)[:4]==bytes([51,102,153,255])
print('PASS UI-only clear/render/readback without HDR/history bindings')
hdr=d.create_texture(size=(1,1,1),format='rgba16float',usage=wgpu.TextureUsage.TEXTURE_BINDING|wgpu.TextureUsage.COPY_DST)
d.queue.write_texture({'texture':hdr},struct.pack('<4e',1.4,0.45,0.12,1),{'bytes_per_row':8,'rows_per_image':1},(1,1,1))
bu=d.create_buffer(size=16,usage=wgpu.BufferUsage.UNIFORM|wgpu.BufferUsage.COPY_DST)
bg=d.create_bind_group(layout=bp.get_bind_group_layout(0),entries=[{'binding':0,'resource':hdr.create_view()},{'binding':1,'resource':d.create_sampler()},{'binding':2,'resource':{'buffer':bu}}])
d.queue.write_buffer(bu,0,struct.pack('<3If',1,0,0,1.0));a=draw(bp,bg)
d.queue.write_buffer(bu,0,struct.pack('<3If',1,1,2,0.5));b=draw(bp,bg)
assert a!=b
print('PASS live 16-byte presentation uniform updates change output:',list(a[:4]),'->',list(b[:4]))
print('LIMIT: Rust unit tests, native application build and real-window input/GPU allocation behavior are NOT executed here.')
