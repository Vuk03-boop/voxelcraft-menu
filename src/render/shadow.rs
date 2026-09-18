use wgpu::*;
pub fn sampled(binding:u32)->BindGroupLayoutEntry {
    BindGroupLayoutEntry { binding, visibility:ShaderStages::COMPUTE,
        ty:BindingType::Texture { sample_type:TextureSampleType::Float { filterable:false }, view_dimension:TextureViewDimension::D2, multisampled:false }, count:None }
}
fn stored(binding:u32,format:TextureFormat)->BindGroupLayoutEntry {
    BindGroupLayoutEntry { binding, visibility:ShaderStages::COMPUTE,
        ty:BindingType::StorageTexture { access:StorageTextureAccess::WriteOnly, format, view_dimension:TextureViewDimension::D2 }, count:None }
}
pub fn layouts(device:&Device)->[BindGroupLayout;3] {
    let entries=[vec![stored(1,TextureFormat::R16Float)],
        vec![sampled(2),stored(3,TextureFormat::Rg16Float)],
        vec![sampled(2),sampled(4),stored(5,TextureFormat::R8Unorm)]];
    std::array::from_fn(|i|device.create_bind_group_layout(&BindGroupLayoutDescriptor { label:Some("shadow stage layout"),entries:&entries[i] }))
}
pub struct Targets {
    pub size:(u32,u32),
    _textures:[Texture;3],
    pub read_group:BindGroup,
    pub groups:[BindGroup;3],
}
impl Targets {
    pub fn new(device:&Device,size:(u32,u32),read:&BindGroupLayout,layouts:&[BindGroupLayout;3])->Self {
        let formats=[TextureFormat::R16Float,TextureFormat::Rg16Float,TextureFormat::R8Unorm];
        let names=["shadow blocker distance R16F","shadow horizontal occlusion/radius RG16F","shadow visibility R8"];
        let textures:[Texture;3]=std::array::from_fn(|i|device.create_texture(&TextureDescriptor {
            label:Some(names[i]),size:Extent3d { width:size.0,height:size.1,depth_or_array_layers:1 },
            mip_level_count:1,sample_count:1,dimension:TextureDimension::D2,format:formats[i],
            usage:TextureUsages::STORAGE_BINDING|TextureUsages::TEXTURE_BINDING,view_formats:&[],
        }));
        let views:[TextureView;3]=std::array::from_fn(|i|textures[i].create_view(&Default::default()));
        let entry=|binding,index:usize|BindGroupEntry { binding,resource:BindingResource::TextureView(&views[index]) };
        let read_group=device.create_bind_group(&BindGroupDescriptor { label:Some("shadow resolve visibility"),layout:read,entries:&[entry(0,2)] });
        let groups=[
            device.create_bind_group(&BindGroupDescriptor {label:Some(names[0]),layout:&layouts[0],entries:&[entry(1,0)]}),
            device.create_bind_group(&BindGroupDescriptor {label:Some(names[1]),layout:&layouts[1],entries:&[entry(2,0),entry(3,1)]}),
            device.create_bind_group(&BindGroupDescriptor {label:Some(names[2]),layout:&layouts[2],entries:&[entry(2,0),entry(4,1),entry(5,2)]}),
        ];
        Self { size,_textures:textures,read_group,groups }
    }
    pub fn bytes(&self)->u64 { u64::from(self.size.0)*u64::from(self.size.1)*7 }
}
