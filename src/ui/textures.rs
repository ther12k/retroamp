//! GPU Texture caching for skin sprites.

use crate::skin::Skin;
use egui::{Context, TextureHandle, TextureOptions};

pub struct SkinTextures {
    pub main: TextureHandle,
    pub cbuttons: TextureHandle,
    pub titlebar: TextureHandle,
    pub numbers: TextureHandle,
    pub text: TextureHandle,
    pub volume: TextureHandle,
    pub balance: TextureHandle,
    pub posbar: TextureHandle,
    pub playpaus: TextureHandle,
    pub shufrep: TextureHandle,
    pub monoster: TextureHandle,
    pub eqmain: TextureHandle,
    pub eq_ex: TextureHandle,
    pub pledit: TextureHandle,
}

impl SkinTextures {
    pub fn new(ctx: &Context, skin: &Skin) -> Self {
        // Pixel-perfect nearest neighbor filtering for retro skin bitmaps
        let options = TextureOptions::NEAREST;

        Self {
            main: ctx.load_texture("skin_main", skin.main.clone(), options),
            cbuttons: ctx.load_texture("skin_cbuttons", skin.cbuttons.clone(), options),
            titlebar: ctx.load_texture("skin_titlebar", skin.titlebar.clone(), options),
            numbers: ctx.load_texture("skin_numbers", skin.numbers.clone(), options),
            text: ctx.load_texture("skin_text", skin.text.clone(), options),
            volume: ctx.load_texture("skin_volume", skin.volume.clone(), options),
            balance: ctx.load_texture("skin_balance", skin.balance.clone(), options),
            posbar: ctx.load_texture("skin_posbar", skin.posbar.clone(), options),
            playpaus: ctx.load_texture("skin_playpaus", skin.playpaus.clone(), options),
            shufrep: ctx.load_texture("skin_shufrep", skin.shufrep.clone(), options),
            monoster: ctx.load_texture("skin_monoster", skin.monoster.clone(), options),
            eqmain: ctx.load_texture("skin_eqmain", skin.eqmain.clone(), options),
            eq_ex: ctx.load_texture("skin_eq_ex", skin.eq_ex.clone(), options),
            pledit: ctx.load_texture("skin_pledit", skin.pledit.clone(), options),
        }
    }

    pub fn update(&mut self, skin: &Skin) {
        self.main.set(skin.main.clone(), TextureOptions::NEAREST);
        self.cbuttons.set(skin.cbuttons.clone(), TextureOptions::NEAREST);
        self.titlebar.set(skin.titlebar.clone(), TextureOptions::NEAREST);
        self.numbers.set(skin.numbers.clone(), TextureOptions::NEAREST);
        self.text.set(skin.text.clone(), TextureOptions::NEAREST);
        self.volume.set(skin.volume.clone(), TextureOptions::NEAREST);
        self.balance.set(skin.balance.clone(), TextureOptions::NEAREST);
        self.posbar.set(skin.posbar.clone(), TextureOptions::NEAREST);
        self.playpaus.set(skin.playpaus.clone(), TextureOptions::NEAREST);
        self.shufrep.set(skin.shufrep.clone(), TextureOptions::NEAREST);
        self.monoster.set(skin.monoster.clone(), TextureOptions::NEAREST);
        self.eqmain.set(skin.eqmain.clone(), TextureOptions::NEAREST);
        self.eq_ex.set(skin.eq_ex.clone(), TextureOptions::NEAREST);
        self.pledit.set(skin.pledit.clone(), TextureOptions::NEAREST);
    }
}
