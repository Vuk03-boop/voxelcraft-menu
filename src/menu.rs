use crate::render::ui::UiRenderer;
use crate::settings::{Grade, Preferences};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen { Title, Playing, Pause, Settings, Loading, ConfirmQuit }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page { Display, Graphics, Appearance, Experimental, Effects }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting { Scale, Fov, Vsync, Taa, Shadows, Ao, Reflections, Refraction, Tone, Grade, Strength, Fog, Ambient, CompactShade, IsolateGlass, ShadowPass, GlassReflect, WaterLook, WindSway, SoftShadows }
impl Setting {
    pub fn experimental(self) -> bool {
        matches!(self, Self::CompactShade | Self::IsolateGlass | Self::ShadowPass | Self::GlassReflect | Self::WaterLook | Self::WindSway | Self::SoftShadows)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Play, Resume, Settings, Quit, QuitAnyway, Back, Tab(Page), Adjust(Setting,i32), Apply, Cancel, Defaults }
#[derive(Clone, Debug)]
pub struct Item { pub rect: [f32;4], pub label: String, pub action: Action }
impl Item {
    pub fn contains(&self,p:[f32;2]) -> bool { p[0]>=self.rect[0] && p[1]>=self.rect[1] && p[0]<self.rect[0]+self.rect[2] && p[1]<self.rect[1]+self.rect[3] }
}

pub struct Menu {
    pub screen: Screen,
    pub page: Page,
    pub draft: Preferences,
    pub focus: usize,
    pub status: String,
    pub save_hint: String,
    pub experiments_locked: bool,
    return_to: Screen,
    quit_return_to: Screen,
}
impl Menu {
    pub fn new(p: Preferences) -> Self { Self { screen: Screen::Title, page: Page::Display, draft:p, focus:0, status:String::new(), save_hint:String::new(), experiments_locked:false, return_to:Screen::Title, quit_return_to:Screen::Title } }
    pub fn open_settings(&mut self,p:&Preferences) { self.return_to=self.screen; self.draft=p.clone(); self.screen=Screen::Settings; self.focus=self.page as usize; self.status.clear(); }
    pub fn cancel(&mut self) { self.screen=self.return_to; self.focus=0; self.status.clear(); }
    pub fn pause(&mut self) { self.screen=Screen::Pause; self.focus=0; self.status.clear(); }
    pub fn confirm_quit(&mut self) {
        if self.screen != Screen::ConfirmQuit { self.quit_return_to=self.screen; }
        self.screen=Screen::ConfirmQuit; self.focus=0;
    }
    pub fn dismiss_quit(&mut self) { self.screen=self.quit_return_to; self.focus=0; }
    pub fn rows(&self) -> Vec<(Setting,&'static str,String)> {
        let p=&self.draft;
        let on=|v| if v { "On".to_string() } else { "Off".to_string() };
        match self.page {
            Page::Display => vec![(Setting::Scale,"Render resolution",format!("{:.0}%",p.scale*100.0)),(Setting::Fov,"Field of view",format!("{:.0} deg",p.fov)),(Setting::Vsync,"VSync",on(p.vsync)),(Setting::Taa,"Temporal antialiasing",on(p.taa))],
            Page::Graphics => vec![(Setting::Shadows,"Sun shadows",on(p.shadows)),(Setting::Ao,"Ambient occlusion",on(p.ao)),(Setting::Reflections,"Water reflections",on(p.reflections)),(Setting::Refraction,"Water transparency",on(p.refraction))],
            Page::Appearance => vec![(Setting::Tone,"Tone mapping",if p.aces {"ACES".into()} else {"Original".into()}),(Setting::Grade,"Colour grade",p.grade.label().into()),(Setting::Strength,"Grade strength",format!("{:.0}%",p.grade_strength*100.0)),(Setting::Fog,"Fog density",format!("{:.5}",p.fog_density)),(Setting::Ambient,"Ambient light",format!("{:.2}",p.ambient))],
            Page::Experimental => vec![
                (Setting::CompactShade,"Compact shade_hit / A-B",on(p.compact_shade_hit)),
                (Setting::SoftShadows,"Soft shadows / screen space",on(p.soft_shadows)),
            ],
            Page::Effects => vec![
                (Setting::GlassReflect,"Glass reflection / effect",on(p.glass_reflect)),
                (Setting::WaterLook,"Water look / existing",on(p.water_look)),
                (Setting::WindSway,"Wind sway / known UV bug",on(p.wind_sway)),
            ],
        }
    }
    pub fn items(&self) -> Vec<Item> {
        let mut v=Vec::new();
        let mut add=|x,y,w,label:String,action| v.push(Item {rect:[x,y,w,42.0],label,action});
        match self.screen {
            Screen::Title => {
                add(260.0,268.0,380.0,"Play".into(),Action::Play);
                add(260.0,326.0,380.0,"Settings".into(),Action::Settings);
                add(260.0,384.0,380.0,"Quit".into(),Action::Quit);
            }
            Screen::Pause => {
                add(260.0,268.0,380.0,"Resume".into(),Action::Resume);
                add(260.0,326.0,380.0,"Settings".into(),Action::Settings);
                add(260.0,384.0,380.0,"Quit game".into(),Action::Quit);
            }
            Screen::Settings => {
                for (i,(page,label)) in [(Page::Display,"Display"),(Page::Graphics,"Graphics"),(Page::Appearance,"Appearance"),(Page::Experimental,"Render lab"),(Page::Effects,"Effects lab")].into_iter().enumerate() {
                    add(100.0+i as f32*141.0,172.0,132.0,label.into(),Action::Tab(page));
                }
                for (i,(setting,_,value)) in self.rows().into_iter().enumerate() {
                    let y=246.0+i as f32*54.0;
                    add(580.0,y,40.0,"-".into(),Action::Adjust(setting,-1));
                    add(626.0,y,114.0,value,Action::Adjust(setting,1));
                    add(746.0,y,40.0,"+".into(),Action::Adjust(setting,1));
                }
                add(100.0,584.0,224.0,"Restore defaults".into(),Action::Defaults);
                add(336.0,584.0,224.0,"Cancel / Back".into(),Action::Cancel);
                add(572.0,584.0,224.0,"Apply and save".into(),Action::Apply);
            }
            Screen::ConfirmQuit => {
                add(220.0,340.0,460.0,"Back - keep playing".into(),Action::Back);
                add(220.0,402.0,460.0,"Quit without saving world".into(),Action::QuitAnyway);
            }
            Screen::Playing|Screen::Loading => {}
        }
        v
    }
    pub fn adjust(&mut self,s:Setting,d:i32) {
        if self.experiments_locked && s.experimental() {
            self.status="Shader options are locked during play. Restart, then change them on the title.".into();
            return;
        }
        let p=&mut self.draft; let d=d as f32;
        match s {
            Setting::Scale=>p.scale=((p.scale+d*0.05)*100.0).round().clamp(25.0,100.0)/100.0,
            Setting::Fov=>p.fov=(p.fov+d*5.0).clamp(30.0,120.0),
            Setting::IsolateGlass=>p.isolate_glass=!p.isolate_glass,
            Setting::ShadowPass=>p.shadow_pass=true,
            Setting::CompactShade=>p.compact_shade_hit=!p.compact_shade_hit,
            Setting::GlassReflect=>p.glass_reflect=!p.glass_reflect,
            Setting::WaterLook=>p.water_look=!p.water_look,
            Setting::WindSway=>p.wind_sway=!p.wind_sway,
            Setting::SoftShadows=>p.soft_shadows=!p.soft_shadows,
            Setting::Vsync=>p.vsync=!p.vsync, Setting::Taa=>p.taa=!p.taa,
            Setting::Shadows=>p.shadows=!p.shadows, Setting::Ao=>p.ao=!p.ao,
            Setting::Reflections=>p.reflections=!p.reflections, Setting::Refraction=>p.refraction=!p.refraction,
            Setting::Tone=>p.aces=!p.aces,
            Setting::Grade=>p.grade=match (p.grade.index() as i32+d as i32).rem_euclid(3) {0=>Grade::None,1=>Grade::Warm,_=>Grade::Cine},
            Setting::Strength=>p.grade_strength=((p.grade_strength+d*0.1)*10.0).round().clamp(0.0,10.0)/10.0,
            Setting::Fog=>p.fog_density=((p.fog_density+d*0.0001)*10000.0).round().clamp(0.0,100.0)/10000.0,
            Setting::Ambient=>p.ambient=((p.ambient+d*0.01)*100.0).round().clamp(0.0,50.0)/100.0,
        }
        self.status="Draft only. Apply to use these settings.".into();
    }
    pub fn restore_defaults(&mut self) {
        let mut defaults=Preferences::default();
        if self.experiments_locked { defaults.keep_experiments_from(&self.draft); }
        self.draft=defaults;
        self.status=if self.experiments_locked {"Draft defaults restored; locked shader options kept."}
            else {"Draft defaults restored, including experiments OFF. Apply to keep."}.into();
    }
    pub fn local_position(p:[f32;2],size:(u32,u32)) -> [f32;2] {
        let s=(size.0 as f32/900.0).min(size.1 as f32/700.0).max(0.001);
        [(p[0]-(size.0 as f32-900.0*s)*0.5)/s,(p[1]-(size.1 as f32-700.0*s)*0.5)/s]
    }
    pub fn hover(&mut self,p:[f32;2],size:(u32,u32)) {
        let local=Self::local_position(p,size);
        if let Some(i)=self.items().iter().position(|item|item.contains(local)) {self.focus=i;}
    }
    pub fn click(&self,p:[f32;2],size:(u32,u32)) -> Option<Action> {
        let local=Self::local_position(p,size);self.items().iter().find(|item|item.contains(local)).map(|i|i.action)
    }
    pub fn key(&mut self,key:KeyCode) -> Option<Action> {
        if key==KeyCode::Escape {return match self.screen { Screen::Settings=>Some(Action::Cancel),Screen::Pause=>Some(Action::Resume),Screen::ConfirmQuit=>Some(Action::Back),_=>None };}
        let items=self.items();let n=items.len();if n==0{return None;}
        self.focus%=n;
        match key {
            KeyCode::ArrowUp=>self.focus=(self.focus+n-1)%n,
            KeyCode::ArrowDown|KeyCode::Tab=>self.focus=(self.focus+1)%n,
            KeyCode::Enter|KeyCode::Space=>return Some(items[self.focus].action),
            KeyCode::ArrowLeft|KeyCode::ArrowRight=>if let Action::Adjust(s,_)=items[self.focus].action {return Some(Action::Adjust(s,if key==KeyCode::ArrowLeft {-1}else{1}));},
            _=>{}
        }
        None
    }
    pub fn draw(&self,ui:&mut UiRenderer,size:(u32,u32),backdrop:bool) {
        ui.verts.clear();
        ui.rect(0.0,0.0,size.0 as f32,size.1 as f32,if backdrop {[0.008,0.018,0.023,0.86]}else{[0.018,0.035,0.043,1.0]});
        let s=(size.0 as f32/900.0).min(size.1 as f32/700.0).max(0.001);
        let ox=(size.0 as f32-900.0*s)*0.5;let oy=(size.1 as f32-700.0*s)*0.5;
        let text=|ui:&mut UiRenderer,x,y,scale,label:&str,color|ui.text(ox+x*s,oy+y*s,scale*s,label,color);
        let pale=[0.87,0.94,0.91,1.0];let muted=[0.49,0.65,0.64,1.0];let gold=[0.95,0.75,0.4,1.0];
        ui.rect(ox+100.0*s,oy+91.0*s,44.0*s,5.0*s,gold);
        text(ui,100.0,48.0,2.0,"VOXELCRAFT / FIELD NOTES",muted);
        let title=match self.screen {Screen::Title=>"VOXELCRAFT",Screen::Pause=>"TAKE A BREATH",Screen::Settings=>"MAKE IT YOURS",Screen::Loading=>"PREPARING WORLD",Screen::ConfirmQuit=>"BEFORE YOU LEAVE",Screen::Playing=>""};
        text(ui,100.0,113.0,5.0,title,pale);
        if self.screen==Screen::Settings {
            for (i,(_,label,_)) in self.rows().iter().enumerate() {text(ui,112.0,260.0+i as f32*54.0,2.5,label,pale);}
            if matches!(self.page, Page::Experimental | Page::Effects) {
                text(ui,110.0,530.0,1.8,if self.experiments_locked {"LOCKED DURING PLAY - restart to change these on the title."} else {"OPT-IN / TITLE ONLY - Play compiles variants. Startup can stall."},gold);
                text(ui,110.0,553.0,1.7,if self.page == Page::Experimental {"Shadow pass also splits water/glass. Extra memory/passes; no speed guarantee."} else {"Effects are not bug fixes. Existing glass/wind defects remain."},muted);
            } else {
                text(ui,110.0,538.0,1.8,"World recipes stay on the CLI. Shader experiments use title-only lab tabs.",muted);
            }
        } else {
            let subtitle=match self.screen {Screen::Pause=>"GAMEPLAY AND WEATHER ARE PAUSED",Screen::Loading=>"Compiling pipelines. First launch can take a moment.",Screen::ConfirmQuit=>if self.status.is_empty() {"This session has no world save destination."} else {"World saving failed. Go back or explicitly discard."},_=>"A WORLD BUILT ONE BLOCK AT A TIME"};
            text(ui,100.0,181.0,2.0,subtitle,muted);
            if self.screen != Screen::ConfirmQuit {text(ui,100.0,480.0,1.8,&self.save_hint,muted);}
        }
        for (i,item) in self.items().iter().enumerate() {
            let selected=i==self.focus || matches!(item.action,Action::Tab(p) if p==self.page);
            let [x,y,w,h]=item.rect;
            ui.rect(ox+x*s,oy+y*s,w*s,h*s,if selected {[0.16,0.29,0.29,1.0]}else{[0.055,0.10,0.115,1.0]});
            if selected {ui.rect(ox+x*s,oy+y*s,3.0*s,h*s,gold);}
            let scale=if w<120.0 {1.8}else{2.1};
            let tw=item.label.len() as f32*4.0*scale;
            text(ui,x+(w-tw)*0.5,y+16.0,scale,&item.label,if selected {pale}else{muted});
        }

        let status:String=self.status.chars().map(|c| if c.is_ascii() {c} else {'?'}).take(95).collect();
        text(ui,100.0,650.0,1.8,&status,gold);
        text(ui,100.0,678.0,1.5,"MOUSE OR ARROW KEYS / ENTER SELECTS / ESC GOES BACK",muted);
    }
}

pub fn present_ui(gpu:&crate::render::Gpu,ui:&mut UiRenderer,target:&wgpu::TextureView,size:(u32,u32)) {
    let mut enc=gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("menu")});
    {
        let mut pass=enc.begin_render_pass(&wgpu::RenderPassDescriptor{
            label:Some("menu"),color_attachments:&[Some(wgpu::RenderPassColorAttachment{
                view:target,depth_slice:None,resolve_target:None,
                ops:wgpu::Operations{load:wgpu::LoadOp::Clear(wgpu::Color::BLACK),store:wgpu::StoreOp::Store},
            })],..Default::default()
        });
        ui.draw(&gpu.queue,&mut pass,size);
    }
    gpu.queue.submit(Some(enc.finish()));
}
