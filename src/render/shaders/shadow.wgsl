@group(3) @binding(1) var shadow_dist_tex: texture_storage_2d<r16float, write>;
@group(3) @binding(2) var shadow_dist_sample: texture_2d<f32>;
@group(3) @binding(3) var shadow_h_tex: texture_storage_2d<rg16float, write>;
@group(3) @binding(4) var shadow_h_sample: texture_2d<f32>;
@group(3) @binding(5) var shadow_mask_tex: texture_storage_2d<r8unorm, write>;
override SHADOW_RADIUS_LIMIT: u32 = 4u;
struct ShadowGuide { hit:vec3<f32>, n:vec3<f32>, t:f32, scale:f32, valid:bool };
fn shadow_guide(p:vec2<i32>) -> ShadowGuide {
    var g:ShadowGuide;
    g.valid=false;
    if any(p<vec2<i32>(0)) || any(p>=vec2<i32>(frame.res)) { return g; }
    let key=atomicLoad(&vis[u32(p.y)*frame.res.x+u32(p.x)]);
    if key==0lu { return g; }
    let ci=u32(key>>24u)&0x1FFFu;
    let low=u32(key&0xFFFFFFlu);
    let v=vec3<u32>((low>>16u)&VOXEL_MASK,(low>>8u)&VOXEL_MASK,low&VOXEL_MASK);
    let id=block_at(ci,v);
    if id==WATER_ID || (SPEC_GLASS && id==GLASS_ID) { return g; }
    let nid=u32(key>>37u)&7u;
    let micro=vec3<u32>((low>>22u)&3u,(low>>14u)&3u,(low>>6u)&3u);
    let rd=ray_dir(f32(p.x)+0.5,f32(p.y)+0.5);
    g.n=normal_of(nid);
    if nid>=NORMAL_CROSS_A && dot(g.n,rd)>0.0 { g.n=-g.n; }
    if dot(g.n,frame.sun_dir)<=0.0 { return g; }
    g.t=hit_t(ci,v,micro,nid,rd);
    g.hit=frame.cam_pos+rd*g.t;
    g.scale=chunks[ci].voxel_size;
    g.valid=g.t>=0.0;
    return g;
}
fn shadow_active()->bool {
    return (frame.flags&FLAG_SHADOWS)!=0u && (frame.flags&FLAG_HEATMAP)==0u && frame.daylight>0.01;
}
@compute @workgroup_size(16,8,1)
fn primary_shadow(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=frame.res) { return; }
    let p=vec2<i32>(gid.xy);
    var distance=0.0;
    if shadow_active() {
        let g=shadow_guide(p);
        if g.valid {

            let blocker=shadow_ray_t(g.hit+g.n*(0.02*g.scale),frame.sun_dir,frame.shadow_dist);
            if blocker>=0.0 { distance=clamp(blocker,0.001,65504.0); }
        }
    }
    textureStore(shadow_dist_tex,p,vec4<f32>(distance,0.0,0.0,0.0));
}
fn shadow_radius(distance:f32,depth:f32)->f32 {
    let proj=f32(frame.res.y)/(2.0*max(frame.tan_half_fov,0.001));
    let r=distance*SUN_ANGLE*SOFT_PENUMBRA*proj/max(depth,0.01);

    return select(0.0,min(r,f32(SHADOW_RADIUS_LIMIT)),r>=0.5);
}
fn shadow_compatible(a:ShadowGuide,b:ShadowGuide)->bool {
    if !b.valid || dot(a.n,b.n)<0.95 { return false; }

    let tolerance=max(0.01,0.01*min(a.scale,b.scale));
    return abs(dot(b.hit-a.hit,a.n))<=tolerance && abs(dot(b.hit-a.hit,b.n))<=tolerance;
}
@compute @workgroup_size(16,8,1)
fn shadow_horizontal(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=frame.res) { return; }
    let p=vec2<i32>(gid.xy);
    let d=textureLoad(shadow_dist_sample,p,0).x;
    let occ=select(0.0,1.0,d>0.0);
    var result=vec2<f32>(occ,0.0);
    if SPEC_SOFT_SHADOWS && shadow_active() {
        let center=shadow_guide(p);
        if center.valid {
            let rc=shadow_radius(d,center.t);
            var sum=occ; var weight=1.0; var support=rc;
            for(var k=-i32(SHADOW_RADIUS_LIMIT);k<=i32(SHADOW_RADIUS_LIMIT);k++) {
                if k==0 { continue; }
                let q=p+vec2<i32>(k,0);
                let guide=shadow_guide(q);
                if !shadow_compatible(center,guide) { continue; }
                let nd=textureLoad(shadow_dist_sample,q,0).x;
                let r=max(rc,shadow_radius(nd,guide.t));

                if r<abs(f32(k)) { continue; }
                let w=exp(-2.0*f32(k*k)/max(r*r,0.25));
                sum+=select(0.0,1.0,nd>0.0)*w; weight+=w; support=max(support,r);
            }
            result=vec2<f32>(sum/weight,support);
        }
    }
    textureStore(shadow_h_tex,p,vec4<f32>(result,0.0,0.0));
}
@compute @workgroup_size(16,8,1)
fn shadow_vertical(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=frame.res) { return; }
    let p=vec2<i32>(gid.xy);
    let h=textureLoad(shadow_h_sample,p,0).xy;
    var occ=h.x;
    if SPEC_SOFT_SHADOWS && shadow_active() {
        let center=shadow_guide(p);
        if center.valid {
            var sum=h.x; var weight=1.0;
            for(var k=-i32(SHADOW_RADIUS_LIMIT);k<=i32(SHADOW_RADIUS_LIMIT);k++) {
                if k==0 { continue; }
                let q=p+vec2<i32>(0,k);
                let guide=shadow_guide(q);
                if !shadow_compatible(center,guide) { continue; }
                let other=textureLoad(shadow_h_sample,q,0).xy;
                let r=max(h.y,other.y);
                if r<abs(f32(k)) { continue; }
                let w=exp(-2.0*f32(k*k)/max(r*r,0.25));
                sum+=other.x*w; weight+=w;
            }
            occ=sum/weight;
        }
    }

    textureStore(shadow_mask_tex,p,vec4<f32>(clamp(1.0-occ,0.0,1.0),0.0,0.0,1.0));
}
