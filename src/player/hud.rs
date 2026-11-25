use bevy::{input::mouse::AccumulatedMouseScroll,prelude::*};
use super::controller::{GameplayActive,PlayerStamina,STAMINA_MAX};
#[derive(Component)]pub(super) struct StaminaHud;#[derive(Component)]pub(super) struct StaminaBarFill;
#[derive(Resource,Default)]pub(super) struct InventorySelection{selected:usize}
#[derive(Component)]pub(super) struct InventorySlotFrame{index:usize}
pub(super)fn spawn_hud(mut commands:Commands,asset_server:Res<AssetServer>){
 commands.init_resource::<InventorySelection>();
 commands.spawn((Node{position_type:PositionType::Absolute,left:px(24),top:px(24),flex_direction:FlexDirection::Column,align_items:AlignItems::Start,row_gap:px(7),..default()},Name::new("Player Status HUD"))).with_children(|hud|{
 hud.spawn((ImageNode::new(asset_server.load("ui/hud/character_portrait.png")),Node{width:px(64),height:px(64),..default()},Name::new("Player Portrait")));
 hud.spawn((Text::new("STAMINA"),TextFont{font_size:FontSize::Px(11.0),..default()},TextColor(Color::srgb(0.68,0.84,0.78))));
 hud.spawn((Node{width:px(132),height:px(7),..default()},BackgroundColor(Color::srgba(0.06,0.10,0.11,0.92)))).with_child((StaminaBarFill,Node{width:percent(100),height:percent(100),..default()},BackgroundColor(Color::srgb(0.36,0.78,0.61))));
 hud.spawn((StaminaHud,Text::new("100%"),TextFont{font_size:FontSize::Px(11.0),..default()},TextColor(Color::srgb(0.68,0.84,0.78))));
 });
 commands.spawn((Node{position_type:PositionType::Absolute,left:percent(50),bottom:px(24),margin:UiRect::left(px(-88)),flex_direction:FlexDirection::Row,column_gap:px(5),..default()},Name::new("Quick Inventory HUD"))).with_children(|inventory|{
 for index in 0..4{inventory.spawn((Node{position_type:PositionType::Relative,width:px(36),height:px(36),align_items:AlignItems::Center,justify_content:JustifyContent::Center,border:UiRect::all(px(1)),..default()},BackgroundColor(Color::srgba(0.025,0.04,0.05,0.82)),BorderColor::all(Color::srgba(0.55,0.65,0.66,0.65)),Name::new(format!("Inventory Slot {}",index+1)))).with_children(|slot|{
 slot.spawn((InventorySlotFrame{index},ImageNode::new(asset_server.load("ui/hud/inventory_slot_selected.png")),Node{position_type:PositionType::Absolute,left:px(-1),right:px(-1),top:px(-1),bottom:px(-1),..default()},if index==0{Visibility::Visible}else{Visibility::Hidden}));
 slot.spawn((Text::new((index+1).to_string()),TextFont{font_size:FontSize::Px(10.0),..default()},TextColor(Color::srgb(0.85,0.89,0.87))));
 });}
 });
}
pub(super)fn update_stamina_hud(stamina:Res<PlayerStamina>,mut labels:Query<&mut Text,With<StaminaHud>>,mut bars:Query<&mut Node,With<StaminaBarFill>>){
 let value=(stamina.current/STAMINA_MAX*100.0).clamp(0.0,100.0);
 for mut label in &mut labels{**label=format!("{value:>3.0}%");}
 for mut bar in &mut bars{bar.width=percent(value);}
}
pub(super)fn update_inventory_selection(scroll:Res<AccumulatedMouseScroll>,active:Res<GameplayActive>,mut selection:ResMut<InventorySelection>){
 if !active.0||scroll.delta.y.abs()<0.01{return;}let direction=if scroll.delta.y>0.0{-1_i32}else{1_i32};selection.selected=(selection.selected as i32+direction).rem_euclid(4)as usize;
}
pub(super)fn sync_inventory_selection(selection:Res<InventorySelection>,mut frames:Query<(&InventorySlotFrame,&mut Visibility)>){
 if !selection.is_changed(){return;}for(frame,mut visibility)in &mut frames{*visibility=if frame.index==selection.selected{Visibility::Visible}else{Visibility::Hidden};}
}