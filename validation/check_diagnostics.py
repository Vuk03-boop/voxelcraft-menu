"""Compile diagnostic specializations individually; no native-game/visual claim."""
from pathlib import Path
import wgpu
root=Path(__file__).resolve().parents[1]
source='\n'.join((root/'src/render/shaders'/f'{n}.wgsl').read_text() for n in ['common','tile_select','march','resolve','taa','shaft','shadow'])
a=wgpu.gpu.request_adapter_sync(power_preference='low-power')
d=a.request_device_sync(required_features=['shader-int64','shader-int64-atomic-min-max','texture-adapter-specific-format-features'],required_limits=a.limits)
m=d.create_shader_module(code=source)
for debug in range(1,8):
 lane=0 if debug<=4 else 2
 d.create_compute_pipeline(layout='auto',compute={'module':m,'entry_point':'resolve','constants':{'SURFACE_DEBUG':debug,'SPEC_SHADOW_PASS':True,'RESOLVE_LANE':lane}})
 print('PASS diagnostic shader specialization',debug,'lane',lane,flush=True)
print('Compile-only check; colour presentation and native interaction require local testing.')
