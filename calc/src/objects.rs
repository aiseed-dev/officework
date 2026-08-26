//! **図形と画像、検索と絞り込み。** 盤面の上に載るもの。

use crate::*;

/// 折れ線の束(1本 = 点の並び)。図形の当たり判定に使います。
type 折れ線の束 = Vec<Vec<(f32, f32)>>;

impl Calc {

    /// この格子座標に**このアプリで挿した図形**があるか(上に描かれた順 = 後勝ち)。
    /// 返すのは (番号, 図形の左上px, 右下隅の掴みか)。
    pub(crate) fn shape_at(&self, x: f32, y: f32) -> Option<(usize, (f32, f32), bool)> {
        for (i, sp) in self.sheet().shapes_new.iter().enumerate().rev() {
            let Some((sx, sy)) = self.cell_origin_px(sp.at) else { continue };
            let (sx, sy) = (sx + sp.dx_px, sy + sp.dy_px);
            let (w, h) = (sp.width_px, sp.height_px);
            if x >= sx && x <= sx + w && y >= sy && y <= sy + h {
                let corner = x >= sx + w - 12.0 && y >= sy + h - 12.0;
                return Some((i, (sx, sy), corner));
            }
        }
        None
    }

    /// 画像(グラフ)の当たり判定。このアプリで挿した分(images_new)だけ —
    /// 読み込んだ画像は原文持ち越しが正なので動かせない(押すとそう言う)
    pub(crate) fn image_at(&self, x: f32, y: f32) -> Option<(usize, (f32, f32), bool)> {
        for (i, im) in self.sheet().images_new.iter().enumerate().rev() {
            let Some((sx, sy)) = self.cell_origin_px(im.at) else { continue };
            let (sx, sy) = (sx + im.dx_px, sy + im.dy_px);
            let (w, h) = (im.width_px, im.height_px);
            if x >= sx && x <= sx + w && y >= sy && y <= sy + h {
                let corner = x >= sx + w - 12.0 && y >= sy + h - 12.0;
                return Some((i, (sx, sy), corner));
            }
        }
        None
    }

    /// 読み込んだ画像(動かせない方)の上か
    pub(crate) fn read_image_at(&self, x: f32, y: f32) -> bool {
        self.sheet().images.iter().any(|im| {
            self.cell_origin_px(im.at).is_some_and(|(sx, sy)| {
                let (sx, sy) = (sx + im.dx_px, sy + im.dy_px);
                x >= sx && x <= sx + im.width_px && y >= sy && y <= sy + im.height_px
            })
        })
    }

    /// 画像のドラッグ(移動 or 右下の掴みで大きさ変更)。図形と同じ作法
    pub(crate) fn image_drag_at(&mut self, x: f32, y: f32, shift: bool) {
        let Some((i, (gx, gy), (ox, oy), resize)) = self.img_drag else { return };
        if self.sheet().images_new.len() <= i {
            return;
        }
        if resize {
            // 比を保って大きさを変える(絵が歪まない)
            let im = &mut self.sheet_mut().images_new[i];
            let ratio = if im.width_px > 0.0 { im.height_px / im.width_px } else { 1.0 };
            im.width_px = (x - ox).max(16.0);
            im.height_px = (im.width_px * ratio).max(16.0);
            let (w, h) = (im.width_px, im.height_px);
            self.dirty = true;
            self.status = ui::tf!("size_px", w, h).into();
        } else {
            // Shift で横か縦に縛る(大きさ変更は元から比を保っている)
            let (mut mx, mut my) = (x - gx, y - gy);
            if shift {
                if mx.abs() >= my.abs() {
                    my = 0.0;
                } else {
                    mx = 0.0;
                }
            }
            let (nx, ny) = (ox + mx, oy + my);
            if let (Some(c), Some(r)) = (self.col_at(nx.max(HEAD_W)), self.row_at(ny.max(ROW_H))) {
                let at = Pos::new(r, c);
                if let Some((cx0, cy0)) = self.cell_origin_px(at) {
                    let (dx, dy) = ((nx - cx0).max(0.0), (ny - cy0).max(0.0));
                    let im = &mut self.sheet_mut().images_new[i];
                    if im.at != at || (im.dx_px - dx).abs() > 0.5 || (im.dy_px - dy).abs() > 0.5 {
                        im.at = at;
                        im.dx_px = dx;
                        im.dy_px = dy;
                        self.dirty = true;
                        self.status = ui::tf!("picture_anchored", at.a1()).into();
                    }
                }
            }
        }
    }

    /// 選んだ図形たちを **1枚の SVG** にまとめる。
    ///
    /// `SheetShape::to_svg` は1つで完結した `<svg>` を返すので、束ねるには
    /// 外側の包みを外して位置をずらして並べ直す。**画素にはしない** —
    /// うちの図形は素が SVG で、PNG に焼くと嘘の解像度が付く(既存の方針)。
    ///
    /// 画面に無い(位置の測れない)図形は入れられないので、入らなかった分は
    /// 呼ぶ側が数えて言う。黙って欠けた図を書き出さない。
    pub(crate) fn shapes_svg(&self, idx: &[usize]) -> String {
        let inner = |svg: &str| -> String {
            // `<svg …>` の閉じ `>` から `</svg>` まで
            match (svg.find('>'), svg.rfind("</svg>")) {
                (Some(a), Some(b)) if b > a => svg[a + 1..b].to_string(),
                _ => String::new(),
            }
        };
        let mut parts: Vec<(f32, f32, f32, f32, String)> = Vec::new();
        for &k in idx {
            let Some(sp) = self.sheet().shapes_new.get(k) else { continue };
            let Some((x, y)) = self.cell_origin_px(sp.at) else { continue };
            parts.push((
                x + sp.dx_px,
                y + sp.dy_px,
                sp.width_px.max(4.0),
                sp.height_px.max(4.0),
                inner(&sp.to_svg()),
            ));
        }
        if parts.len() <= 1 {
            // 1つなら素の姿のまま(余計な包みを足さない)
            return idx
                .first()
                .and_then(|k| self.sheet().shapes_new.get(*k))
                .map(|sp| sp.to_svg())
                .unwrap_or_default();
        }
        let x0 = parts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let y0 = parts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let x1 = parts.iter().map(|p| p.0 + p.2).fold(f32::NEG_INFINITY, f32::max);
        let y1 = parts.iter().map(|p| p.1 + p.3).fold(f32::NEG_INFINITY, f32::max);
        // 影のぶんの余白(to_svg が 4px ずらして描く)を右下に足す
        let (w, h) = ((x1 - x0 + 8.0).max(4.0), (y1 - y0 + 8.0).max(4.0));
        let body: String = parts
            .iter()
            .map(|(x, y, _, _, inner)| {
                format!(r#"<g transform="translate({:.2} {:.2})">{inner}</g>"#, x - x0, y - y0)
            })
            .collect();
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}">{body}</svg>"#
        )
    }

    /// 選んだ画像を消す(Del の実体)
    pub(crate) fn delete_selected_image(&mut self) -> bool {
        let Some(i) = self.img_sel.take() else { return false };
        if self.sheet().images_new.len() <= i {
            return false;
        }
        self.checkpoint();
        self.sheet_mut().images_new.remove(i);
        self.dirty = true;
        self.status = ui::t!("image_deleted_ctrl_z").into();
        true
    }

    /// 図形の右クリックメニューの実体(切り貼り・重なり順・回転・SVG保存)。
    /// window を要らなくしてあるので試験からそのまま呼べる
    pub(crate) fn shape_menu_action(&mut self, id: &str) {
        match id {
            // ポイントの編集(入切)。**点で形を作る図形でだけ**
            "sh-points" => {
                if self.point_edit.is_some() {
                    self.point_edit = None;
                    self.pt_drag = None;
                    self.status = ui::t!("left_point_editing").into();
                    return;
                }
                let Some(i) = self.shape_sel else { return };
                if self.sheet().shapes_new.get(i).map(|s| s.points.len()).unwrap_or(0) < 2 {
                    self.status =
                        ui::t!("shape_no_vertices_use").into();
                    return;
                }
                self.point_edit = Some(i);
                self.status =
                    ui::t!("point_editing_drag_point").into();
            }
            "sh-copy" | "sh-cut" => {
                let Some(i) = self.shape_sel else { return };
                let Some(sp) = self.sheet().shapes_new.get(i).cloned() else { return };
                self.shape_clip = Some(sp);
                if id == "sh-cut" {
                    self.checkpoint();
                    self.sheet_mut().shapes_new.remove(i);
                    self.shape_sel = None;
                    self.point_edit = None;
                    self.shape_multi.clear();
                    self.dirty = true;
                    self.status = ui::t!("cut_shape_paste_brings").into();
                } else {
                    self.status = ui::t!("copied_shape").into();
                }
            }
            "sh-paste" => {
                let Some(mut sp) = self.shape_clip.clone() else {
                    self.status = ui::t!("no_shape_paste_copy").into();
                    return;
                };
                self.checkpoint();
                sp.at = self.cursor;
                (sp.dx_px, sp.dy_px) = (4.0, 4.0);
                self.sheet_mut().shapes_new.push(sp);
                self.shape_sel = Some(self.sheet().shapes_new.len() - 1);
                self.dirty = true;
                self.status = ui::tf!("pasted_shape", self.cursor.a1()).into();
            }
            "sh-del" => {
                // Ctrl+クリックの束ごと消す(Del キーと同じ振る舞い)
                let Some(i) = self.shape_sel.take() else { return };
                let mut idx: Vec<usize> = std::mem::take(&mut self.shape_multi);
                idx.push(i);
                idx.sort_unstable();
                idx.dedup();
                idx.retain(|&k| k < self.sheet().shapes_new.len());
                if idx.is_empty() {
                    return;
                }
                self.checkpoint();
                for k in idx.iter().rev() {
                    self.sheet_mut().shapes_new.remove(*k);
                }
                self.dirty = true;
                self.status = if idx.len() == 1 {
                    ui::t!("shape_deleted_ctrl_z").into()
                } else {
                    ui::tf!("deleted_shapes_ctrl_z", idx.len()).into()
                };
            }
            // 重なり順 = shapes_new の並び(後に描く方が前)。
            // 並びが変わると束の番号が狂うので、束は解いて主の1つに絞る
            "sh-front" | "sh-forward" | "sh-backward" | "sh-back" => {
                self.shape_multi.clear();
                let Some(i) = self.shape_sel else { return };
                let len = self.sheet().shapes_new.len();
                if len <= i {
                    return;
                }
                let j = match id {
                    "sh-front" => len - 1,
                    "sh-forward" => (i + 1).min(len - 1),
                    "sh-backward" => i.saturating_sub(1),
                    _ => 0,
                };
                if i == j {
                    self.status = ui::t!("already_there_shapes_drawn").into();
                    return;
                }
                self.checkpoint();
                let sp = self.sheet_mut().shapes_new.remove(i);
                self.sheet_mut().shapes_new.insert(j, sp);
                self.shape_sel = Some(j);
                self.dirty = true;
                self.status = match id {
                    "sh-front" => ui::t!("moved_front").into(),
                    "sh-forward" => ui::t!("moved_forward").into(),
                    "sh-backward" => ui::t!("moved_backward").into(),
                    _ => ui::t!("moved_back").into(),
                };
            }
            "sh-rot-r" | "sh-rot-l" => {
                let d = if id == "sh-rot-r" { 90.0 } else { -90.0 };
                self.shape_edit(move |sp| sp.rot = (sp.rot + d).rem_euclid(360.0));
                self.status = ui::t!("rotated_90").into();
            }
            "sh-flip-h" => {
                self.shape_edit(|sp| sp.flip_h = !sp.flip_h);
                self.status = ui::t!("flipped_left_right").into();
            }
            "sh-flip-v" => {
                self.shape_edit(|sp| sp.flip_v = !sp.flip_v);
                self.status = ui::t!("flipped_top_bottom").into();
            }
            // 画像として保存 = SVG(うちの図形の素の姿。嘘の PNG 変換はしない)
            "sh-save" => {
                let Some(i) = self.shape_sel else { return };
                if self.sheet().shapes_new.get(i).is_none() {
                    return;
                }
                // **束ねてあれば1枚にまとめる。** SmartArt は「うちの図形の
                // 集まり」として組む設計なので、1つだけ書き出すと図の
                // 一部しか保存できない(2026-08-13、台帳「SmartArt
                // 右クリック『画像として保存』」)
                let mut idx: Vec<usize> = std::iter::once(i)
                    .chain(self.shape_multi.iter().copied())
                    .collect();
                idx.sort_unstable();
                idx.dedup();
                idx.retain(|&k| k < self.sheet().shapes_new.len());
                let svg = self.shapes_svg(&idx);
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("figure.svg")
                    .save_file()
                else {
                    self.status = ui::t!("save_cancelled").into();
                    return;
                };
                self.status = match std::fs::write(&path, svg) {
                    Ok(_) => ui::tf!("saved_svg", path.display().to_string()).into(),
                    Err(e) => ui::tf!("cant_save", e.to_string()).into(),
                };
            }
            // 詳細設定 = 右の設定パネル(選択中はいつも出ている)
            "sh-settings" => {
                self.status = ui::t!("settings_shape_settings_panel").into();
            }
            _ => {}
        }
    }

    /// 選択中の図形に手を入れる(undo 1手ぶんを刻んで)。設定パネルが使う
    pub(crate) fn shape_edit(&mut self, f: impl FnOnce(&mut sheet::model::SheetShape)) {
        let Some(i) = self.shape_sel else { return };
        if self.sheet().shapes_new.len() <= i {
            return;
        }
        self.checkpoint();
        f(&mut self.sheet_mut().shapes_new[i]);
        self.dirty = true;
    }

    /// 図形を格子の絶対 px の位置へ置き直す(アンカーのセル+ずらしに直す)。
    /// 整列・分布が使う。置き先が画面に無ければ動かさない(黙って飛ばさない)
    pub(crate) fn place_shape_px(&mut self, i: usize, nx: f32, ny: f32) -> bool {
        if let (Some(c), Some(r)) = (self.col_at(nx.max(HEAD_W)), self.row_at(ny.max(ROW_H))) {
            let at = Pos::new(r, c);
            if let Some((cx0, cy0)) = self.cell_origin_px(at) {
                let sp = &mut self.sheet_mut().shapes_new[i];
                sp.at = at;
                sp.dx_px = (nx - cx0).max(0.0);
                sp.dy_px = (ny - cy0).max(0.0);
                return true;
            }
        }
        false
    }

    /// 整列と分布(Ctrl+クリックで束ねた図形へ)。整列は2個から、分布は3個から。
    /// 基準は束の外接の箱(本家の「選択した図形に合わせる」と同じ)
    pub(crate) fn shape_align(&mut self, id: &str) {
        let mut idx: Vec<usize> = self
            .shape_sel
            .into_iter()
            .chain(self.shape_multi.iter().copied())
            .collect();
        idx.sort_unstable();
        idx.dedup();
        idx.retain(|&i| i < self.sheet().shapes_new.len());
        // (番号, x, y, w, h)。画面に見えている(=位置が測れる)ものだけ
        let mut items: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
        for &i in &idx {
            let sp = &self.sheet().shapes_new[i];
            if let Some((sx, sy)) = self.cell_origin_px(sp.at) {
                items.push((i, sx + sp.dx_px, sy + sp.dy_px, sp.width_px, sp.height_px));
            }
        }
        let need = if id.starts_with("sh-dist") { 3 } else { 2 };
        if items.len() < need {
            self.status = ui::tf!(
                "select_more_shapes_first",
                need
            )
            .into();
            return;
        }
        self.checkpoint();
        let min_x = items.iter().map(|it| it.1).fold(f32::MAX, f32::min);
        let max_r = items.iter().map(|it| it.1 + it.3).fold(f32::MIN, f32::max);
        let min_y = items.iter().map(|it| it.2).fold(f32::MAX, f32::min);
        let max_b = items.iter().map(|it| it.2 + it.4).fold(f32::MIN, f32::max);
        let mut moves: Vec<(usize, f32, f32)> = Vec::new();
        match id {
            "sh-al-l" => moves.extend(items.iter().map(|&(i, _, y, _, _)| (i, min_x, y))),
            "sh-al-r" => {
                moves.extend(items.iter().map(|&(i, _, y, w, _)| (i, max_r - w, y)))
            }
            "sh-al-c" => {
                let c = (min_x + max_r) / 2.0;
                moves.extend(items.iter().map(|&(i, _, y, w, _)| (i, c - w / 2.0, y)));
            }
            "sh-al-t" => moves.extend(items.iter().map(|&(i, x, _, _, _)| (i, x, min_y))),
            "sh-al-b" => {
                moves.extend(items.iter().map(|&(i, x, _, _, h)| (i, x, max_b - h)))
            }
            "sh-al-m" => {
                let m = (min_y + max_b) / 2.0;
                moves.extend(items.iter().map(|&(i, x, _, _, h)| (i, x, m - h / 2.0)));
            }
            // 分布: 端の2つは留め、間の隙間を等しく
            "sh-dist-h" => {
                items.sort_by(|a, b| a.1.total_cmp(&b.1));
                let sum_w: f32 = items.iter().map(|it| it.3).sum();
                let gap = ((max_r - min_x) - sum_w) / (items.len() - 1) as f32;
                let mut x = min_x;
                for &(i, _, y, w, _) in &items {
                    moves.push((i, x, y));
                    x += w + gap;
                }
            }
            "sh-dist-v" => {
                items.sort_by(|a, b| a.2.total_cmp(&b.2));
                let sum_h: f32 = items.iter().map(|it| it.4).sum();
                let gap = ((max_b - min_y) - sum_h) / (items.len() - 1) as f32;
                let mut y = min_y;
                for &(i, x, _, _, h) in &items {
                    moves.push((i, x, y));
                    y += h + gap;
                }
            }
            _ => return,
        }
        let mut n = 0usize;
        for (i, nx, ny) in moves {
            n += self.place_shape_px(i, nx, ny) as usize;
        }
        self.dirty = true;
        self.status = match id {
            "sh-al-l" => ui::tf!("aligned_left", n).into(),
            "sh-al-c" => ui::tf!("centred_horizontally", n).into(),
            "sh-al-r" => ui::tf!("aligned_right", n).into(),
            "sh-al-t" => ui::tf!("aligned_top", n).into(),
            "sh-al-m" => ui::tf!("centred_vertically", n).into(),
            "sh-al-b" => ui::tf!("aligned_bottom", n).into(),
            "sh-dist-h" => ui::tf!("spread_evenly_across", n).into(),
            _ => ui::tf!("spread_evenly_down", n).into(),
        };
    }

    /// 選択中の図形の回転の取っ手の中心(格子px)。折れ線ものには無い
    pub(crate) fn shape_rot_handle(&self, i: usize) -> Option<(f32, f32)> {
        let sp = self.sheet().shapes_new.get(i)?;
        if matches!(
            sp.kind.as_str(),
            "spark" | "spark-col" | "spark-wl" | "ink" | "marker"
        ) {
            return None;
        }
        let (sx, sy) = self.cell_origin_px(sp.at)?;
        Some((sx + sp.dx_px + sp.width_px / 2.0, sy + sp.dy_px - 18.0))
    }

    /// 回転ドラッグ。真上が0度、ポインタの向きへ時計回り。Shift で15度刻み
    pub(crate) fn shape_rotate_at(&mut self, x: f32, y: f32, snap: bool) {
        let Some(i) = self.shape_rot else { return };
        let Some(sp) = self.sheet().shapes_new.get(i) else { return };
        let Some((sx, sy)) = self.cell_origin_px(sp.at) else { return };
        let (ccx, ccy) = (
            sx + sp.dx_px + sp.width_px / 2.0,
            sy + sp.dy_px + sp.height_px / 2.0,
        );
        let mut deg = (x - ccx).atan2(-(y - ccy)).to_degrees();
        if snap {
            deg = (deg / 15.0).round() * 15.0;
        }
        let deg = deg.rem_euclid(360.0);
        let sp = &mut self.sheet_mut().shapes_new[i];
        if (sp.rot - deg).abs() > 0.01 {
            sp.rot = deg;
            self.dirty = true;
            self.status = ui::tf!("rotation", format!("{deg:.0}")).into();
        }
    }

    /// 図形のドラッグ(移動 or 右下の掴みで大きさ変更)。
    ///
    /// **Shift を押している間は縛る**(2026-08-13、台帳
    /// ManipulateObjects) — 大きさ変更は縦横の比を保ち、移動は横か縦の
    /// どちらかだけに。**どちらに縛るかは、動かした量の大きいほう**で決める
    pub(crate) fn shape_drag_at(&mut self, x: f32, y: f32, shift: bool) {
        let Some((i, (gx, gy), (ox, oy), resize)) = self.shape_drag else { return };
        if self.sheet().shapes_new.len() <= i {
            return;
        }
        if resize {
            let sp = &mut self.sheet_mut().shapes_new[i];
            let ratio = if sp.width_px > 0.0 { sp.height_px / sp.width_px } else { 1.0 };
            sp.width_px = (x - ox).max(16.0);
            sp.height_px = if shift {
                (sp.width_px * ratio).max(16.0)
            } else {
                (y - oy).max(16.0)
            };
            let (w, h) = (sp.width_px, sp.height_px);
            self.dirty = true;
            self.status = ui::tf!("size_px", w, h).into();
        } else {
            // 移動: 掴んだときのずれを保って、左上の来るセルに留め直す。
            // **留め方は place_shape_px に1本化**(整列・分布・Ctrl+矢印と同じ道)
            let before = {
                let sp = &self.sheet().shapes_new[i];
                (sp.at, sp.dx_px, sp.dy_px)
            };
            let (mut mx, mut my) = (x - gx, y - gy);
            if shift {
                // 横か縦か、動かした量の大きいほうだけ通す
                if mx.abs() >= my.abs() {
                    my = 0.0;
                } else {
                    mx = 0.0;
                }
            }
            if self.place_shape_px(i, ox + mx, oy + my) {
                let sp = &self.sheet().shapes_new[i];
                if sp.at != before.0
                    || (sp.dx_px - before.1).abs() > 0.5
                    || (sp.dy_px - before.2).abs() > 0.5
                {
                    let at = sp.at;
                    self.dirty = true;
                    self.status = ui::tf!("shape_anchored", at.a1()).into();
                }
            }
        }
    }

    /// いま選んでいる図形を **1px** 動かす。Ctrl+矢印から呼ぶ。
    ///
    /// **図形を選んでいる間だけ** Ctrl+矢印を奪う(2026-08-13 発注者)。
    /// 素の Ctrl+矢印は「データの端へ」で、表の仕事の芯なので、
    /// 選んでいないときは触らない。動かしたら true — 呼んだ側は
    /// 「端へ」をやめる
    pub(crate) fn nudge_shape(&mut self, dx: f32, dy: f32) -> bool {
        let Some(i) = self.shape_sel else { return false };
        if self.sheet().shapes_new.len() <= i {
            return false;
        }
        let sp = &self.sheet().shapes_new[i];
        let (at, ox, oy) = (sp.at, sp.dx_px, sp.dy_px);
        let Some((cx0, cy0)) = self.cell_origin_px(at) else { return false };
        if self.place_shape_px(i, cx0 + ox + dx, cy0 + oy + dy) {
            let sp = &self.sheet().shapes_new[i];
            let (nat, ndx, ndy) = (sp.at, sp.dx_px, sp.dy_px);
            if nat != at || (ndx - ox).abs() > 0.01 || (ndy - oy).abs() > 0.01 {
                self.dirty = true;
                self.status = ui::tf!("shape_anchored", nat.a1()).into();
            }
        }
        // **1px でも「動かした」と答える。** 左上の端に張り付いて動けなくても、
        // カーソルが表の端まで飛んでいくよりは良い(選んでいるのは図形なので)
        true
    }

    /// あるシートの中の当たり(行→列の順)。式の中の文字も探す
    /// (`editable` = 打った通りの姿)。
    fn 当たり(sh: &sheet::Sheet, term: &str) -> Vec<Pos> {
        sh.cells
            .iter()
            .filter(|(_, c)| c.editable().contains(term) || c.value.display().contains(term))
            .map(|(p, _)| *p)
            .collect()
    }

    /// 「次を検索」。いまのセルの次から探し、末尾まで行ったら頭に戻る。
    ///
    /// **範囲は3つ**(2026-08-20 発注者)。ここは*このシート*と*このファイル*を
    /// 受け持ちます(フォルダ全体は `find_in_folder`)。
    ///
    /// *このファイル*のときは、いまのシートを見終えたら次のシートへ回り、
    /// **見つかったシートへ切り替えてから**カーソルを合わせます —
    /// 「3件見つかりました」と言って動かないのは、見つけていないのと同じです。
    pub(crate) fn find_next(&mut self, term: &str) {
        let ブック全体 = self.find_book;
        let n_sheets = self.book.sheets.len();
        // いまのシートから始めて、ファイル全体なら後ろのシートへ回る
        let 見る順: Vec<usize> = if ブック全体 {
            (0..n_sheets).map(|k| (self.active + k) % n_sheets).collect()
        } else {
            vec![self.active]
        };
        let 総数: usize =
            見る順.iter().map(|i| Self::当たり(&self.book.sheets[*i], term).len()).sum();
        if 総数 == 0 {
            self.status = if ブック全体 {
                ui::tf!("not_file", term).into()
            } else {
                ui::tf!("not_sheet_choose_file", term).into()
            };
            self.find_term = Some(term.to_string());
            return;
        }
        for (k, &i) in 見る順.iter().enumerate() {
            let hits = Self::当たり(&self.book.sheets[i], term);
            if hits.is_empty() {
                continue;
            }
            // いまのシートの続きからだけ「次」を探す。回った先は頭から
            let next = if k == 0 {
                let cur = self.cursor;
                match hits.iter().find(|p| **p > cur).copied() {
                    Some(p) => Some(p),
                    // このシートは見終えた。ファイル全体なら次のシートへ
                    None if ブック全体 && 見る順.len() > 1 => None,
                    None => Some(hits[0]),
                }
            } else {
                Some(hits[0])
            };
            let Some(next) = next else { continue };
            if i != self.active {
                self.active = i;
                self.sheet_ui.clear();
            }
            self.anchor = None;
            self.cursor = next;
            self.follow();
            self.sync_input();
            self.status = if ブック全体 {
                ui::tf!("file",
                        term, self.book.sheets[i].name.clone(), next.a1(), 総数.to_string())
                    .into()
            } else {
                ui::tf!("sheet_3", term, next.a1(), 総数.to_string())
                    .into()
            };
            self.find_term = Some(term.to_string());
            return;
        }
        // 全部見終えて戻ってきた = 先頭の当たりへ
        let i = 見る順[0];
        let hits = Self::当たり(&self.book.sheets[i], term);
        if let Some(&first) = hits.first() {
            self.active = i;
            self.cursor = first;
            self.follow();
            self.sync_input();
        }
        self.find_term = Some(term.to_string());
    }

    /// 絞り込みに一致する行(見出し行 0 は常に入れる)。
    /// オートフィルタで残る行か。範囲の外と見出し行は常に残す
    pub(crate) fn filter_keeps(&self, r: u32) -> bool {
        let Some(f) = &self.auto_filter else { return true };
        let (a, b) = f.range;
        if r <= a.row || r > b.row {
            return true;
        }
        for (col, hide) in &f.hide {
            let v = self
                .sheet()
                .get(Pos::new(r, *col))
                .map(|c| c.value.display())
                .unwrap_or_default();
            if hide.contains(&v) {
                return false;
            }
        }
        true
    }

    /// 絞り込みが実際に効いているか(どれかの列で値を隠している)
    pub(crate) fn filter_active(&self) -> bool {
        self.auto_filter.as_ref().is_some_and(|f| !f.hide.is_empty())
    }

    /// 絞り込みの「n 行中 m 行を表示」(範囲のデータ行で数える)
    pub(crate) fn filter_counts(&self) -> Option<(u32, u32)> {
        if !self.filter_active() {
            return None;
        }
        let (a, b) = self.auto_filter.as_ref()?.range;
        let total = b.row - a.row;
        let shown = ((a.row + 1)..=b.row).filter(|r| self.filter_keeps(*r)).count() as u32;
        Some((total, shown))
    }

    /// ▼のパネルに出す値の一覧(値, 件数)。**他の列の絞り込みは効かせたまま**
    /// この列の値を数える(Excel の作法)。1,000 種で切り、切ったら true
    pub(crate) fn filter_values(&self, col: u32) -> (Vec<(String, usize)>, bool) {
        let Some(f) = &self.auto_filter else { return (Vec::new(), false) };
        let (a, b) = f.range;
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for r in (a.row + 1)..=b.row {
            if self.sheet().row_hidden.contains(&r) {
                continue;
            }
            let mut ok = true;
            for (c2, hide) in &f.hide {
                if *c2 == col {
                    continue;
                }
                let v = self
                    .sheet()
                    .get(Pos::new(r, *c2))
                    .map(|c| c.value.display())
                    .unwrap_or_default();
                if hide.contains(&v) {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }
            let v = self
                .sheet()
                .get(Pos::new(r, col))
                .map(|c| c.value.display())
                .unwrap_or_default();
            *counts.entry(v).or_default() += 1;
        }
        let cut = counts.len() > 1000;
        (counts.into_iter().take(1000).collect(), cut)
    }
}

impl Calc {
    /// ポイント編集の取っ手を全部あげる(格子px)。
    ///
    /// **描く側と押す側で同じ表を使う。** 別々に持つと、見えている丸と
    /// 当たり判定がずれる — 回転の取っ手で一度その形を踏んでいる。
    /// 返りは (点の番号, 種類, x, y)。制御点は**その点が持っているときだけ**
    pub(crate) fn point_handles(&self, i: usize) -> Vec<(usize, PtHandle, f32, f32)> {
        let Some(sp) = self.sheet().shapes_new.get(i) else { return Vec::new() };
        let Some((sx, sy)) = self.cell_origin_px(sp.at) else { return Vec::new() };
        let (ox, oy) = (sx + sp.dx_px, sy + sp.dy_px);
        let (w, h) = (sp.width_px.max(1.0), sp.height_px.max(1.0));
        let ex = |p: (f32, f32)| (ox + p.0 * w, oy + p.1 * h);
        let mut out = Vec::new();
        for (k, pp) in sp.points.iter().enumerate() {
            let (x, y) = ex(pp.at);
            out.push((k, PtHandle::Vertex, x, y));
            if let Some(c) = pp.c_in {
                let (x, y) = ex(c);
                out.push((k, PtHandle::CtrlIn, x, y));
            }
            if let Some(c) = pp.c_out {
                let (x, y) = ex(c);
                out.push((k, PtHandle::CtrlOut, x, y));
            }
        }
        out
    }

    /// その場所にある取っ手(近い順に1つ)。掴む半径は 7px
    pub(crate) fn point_hit(&self, i: usize, x: f32, y: f32) -> Option<(usize, PtHandle)> {
        let mut best: Option<(f32, usize, PtHandle)> = None;
        for (k, kind, hx, hy) in self.point_handles(i) {
            let d = (hx - x).powi(2) + (hy - y).powi(2);
            if d <= 49.0 && best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, k, kind));
            }
        }
        best.map(|(_, k, kind)| (k, kind))
    }

    /// つまんだ取っ手を動かす。座標は格子px → 図形の中の 0..1 へ直す
    pub(crate) fn point_drag_at(&mut self, x: f32, y: f32) {
        let Some(i) = self.point_edit else { return };
        let Some((k, kind)) = self.pt_drag else { return };
        let Some(sp) = self.sheet().shapes_new.get(i) else { return };
        let Some((sx, sy)) = self.cell_origin_px(sp.at) else { return };
        let (ox, oy) = (sx + sp.dx_px, sy + sp.dy_px);
        let (w, h) = (sp.width_px.max(1.0), sp.height_px.max(1.0));
        // **枠の外へは出さない。** 0..1 の外に出ると、xlsx の 10000 目盛りで
        // 負や桁あふれになり、Excel が形を描けなくなる
        let nx = ((x - ox) / w).clamp(0.0, 1.0);
        let ny = ((y - oy) / h).clamp(0.0, 1.0);
        let sp = &mut self.sheet_mut().shapes_new[i];
        let Some(pp) = sp.points.get_mut(k) else { return };
        match kind {
            PtHandle::Vertex => {
                // 頂点を動かすと、その点が持つ制御点も一緒に動く —
                // 曲がり方を保ったまま形だけ動かせる(Illustrator と同じ作法)
                let (dx, dy) = (nx - pp.at.0, ny - pp.at.1);
                pp.at = (nx, ny);
                if let Some(c) = &mut pp.c_in {
                    *c = (c.0 + dx, c.1 + dy);
                }
                if let Some(c) = &mut pp.c_out {
                    *c = (c.0 + dx, c.1 + dy);
                }
            }
            PtHandle::CtrlIn => pp.c_in = Some((nx, ny)),
            PtHandle::CtrlOut => pp.c_out = Some((nx, ny)),
        }
        self.dirty = true;
    }

    /// 頂点を足す/外す(Ctrl+クリック)。
    ///
    /// 取っ手の上なら**外す**、線の上なら**そこへ足す**。
    /// 点が2つのときは外させない — 線でなくなる
    pub(crate) fn point_add_or_remove(&mut self, x: f32, y: f32) -> bool {
        let Some(i) = self.point_edit else { return false };
        if let Some((k, PtHandle::Vertex)) = self.point_hit(i, x, y) {
            let n = self.sheet().shapes_new[i].points.len();
            if n <= 2 {
                self.status = ui::t!("no_fewer_than_line").into();
                return true;
            }
            self.checkpoint();
            self.sheet_mut().shapes_new[i].points.remove(k);
            self.dirty = true;
            self.status = ui::t!("removed_one_vertex").into();
            return true;
        }
        // 線の上: いちばん近い区間の真ん中へ足す
        let Some(sp) = self.sheet().shapes_new.get(i) else { return false };
        let Some((sx, sy)) = self.cell_origin_px(sp.at) else { return false };
        let (ox, oy) = (sx + sp.dx_px, sy + sp.dy_px);
        let (w, h) = (sp.width_px.max(1.0), sp.height_px.max(1.0));
        let mut best: Option<(f32, usize)> = None;
        for k in 1..sp.points.len() {
            let a = sp.points[k - 1].at;
            let b = sp.points[k].at;
            let (ax, ay) = (ox + a.0 * w, oy + a.1 * h);
            let (bx, by) = (ox + b.0 * w, oy + b.1 * h);
            // 点と線分の距離(曲がりは見ない — 足す先は区間で足りる)
            let (vx, vy) = (bx - ax, by - ay);
            let len2 = vx * vx + vy * vy;
            let t = if len2 > 0.0 {
                (((x - ax) * vx + (y - ay) * vy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = (ax + vx * t - x).powi(2) + (ay + vy * t - y).powi(2);
            if d <= 64.0 && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, k));
            }
        }
        let Some((_, k)) = best else { return false };
        self.checkpoint();
        let sp = &mut self.sheet_mut().shapes_new[i];
        let a = sp.points[k - 1].at;
        let b = sp.points[k].at;
        sp.points.insert(
            k,
            sheet::model::PathPoint::at((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
        );
        self.dirty = true;
        self.status = ui::t!("added_one_vertex").into();
        true
    }

    /// その点の曲がりを入切する(取っ手をダブルクリック相当)。
    /// 曲げるときは、両隣へ向けて 1/3 の所に制御点を置く
    pub(crate) fn point_toggle_curve(&mut self, k: usize) {
        let Some(i) = self.point_edit else { return };
        let Some(sp) = self.sheet().shapes_new.get(i) else { return };
        let n = sp.points.len();
        if k >= n {
            return;
        }
        let cur = sp.points[k];
        let prev = sp.points[k.saturating_sub(1)].at;
        let next = sp.points[(k + 1).min(n - 1)].at;
        self.checkpoint();
        let pp = &mut self.sheet_mut().shapes_new[i].points[k];
        if pp.c_in.is_some() || pp.c_out.is_some() {
            pp.c_in = None;
            pp.c_out = None;
            self.status = ui::t!("made_point_corner").into();
        } else {
            let a = cur.at;
            pp.c_in = Some((a.0 + (prev.0 - a.0) / 3.0, a.1 + (prev.1 - a.1) / 3.0));
            pp.c_out = Some((a.0 + (next.0 - a.0) / 3.0, a.1 + (next.1 - a.1) / 3.0));
            self.status = ui::t!("made_point_curve").into();
        }
        self.dirty = true;
    }
}

impl Calc {
    /// 図形どうしの足し引き(台帳「図形のブール演算」、2026-08-13)。
    ///
    /// **2つ選んでいるときだけ。** 主(`shape_sel`)から控え(`shape_multi`)を
    /// 引く、という向きにする — 「どちらから引くか」を選べないと減算が使えない。
    pub(crate) fn shapes_boolean(&mut self, op: sheet::model::BoolOp) {
        use sheet::model::{combine, outline, to_points, BoolOp};
        let (Some(a), Some(&b)) = (self.shape_sel, self.shape_multi.first()) else {
            self.status = ui::t!("select_two_shapes_ctrl").into();
            return;
        };
        let (Some(sa), Some(sb)) = (
            self.sheet().shapes_new.get(a).cloned(),
            self.sheet().shapes_new.get(b).cloned(),
        ) else {
            return;
        };
        // **輪郭を出せない形は断る。** 黙って四角で計算しない
        let (Some(oa), Some(ob)): (Option<折れ線の束>, Option<折れ線の束>) = (
            outline(&sa.kind, &sa.points),
            outline(&sb.kind, &sb.points),
        ) else {
            self.status =
                ui::t!("shape_cannot_combined_outline").into();
            return;
        };
        // 2つ目を1つ目の枠の目盛りへ 直す(画面の px を経由する)
        let (Some((ax, ay)), Some((bx, by))) = (
            self.cell_origin_px(sa.at),
            self.cell_origin_px(sb.at),
        ) else {
            return;
        };
        let (ax, ay) = (ax + sa.dx_px, ay + sa.dy_px);
        let (bx, by) = (bx + sb.dx_px, by + sb.dy_px);
        let (aw, ah) = (sa.width_px.max(1.0), sa.height_px.max(1.0));
        let (bw, bh) = (sb.width_px.max(1.0), sb.height_px.max(1.0));
        let ob: Vec<Vec<(f32, f32)>> = ob
            .iter()
            .map(|c| {
                c.iter()
                    .map(|&(x, y)| {
                        // b の 0..1 → 画面 px → a の 0..1
                        (((bx + x * bw) - ax) / aw, ((by + y * bh) - ay) / ah)
                    })
                    .collect()
            })
            .collect();
        let res = combine(&oa, &ob, op);
        if res.is_empty() {
            self.status = ui::t!("nothing_left_not_overlap").into();
            return;
        }
        self.checkpoint();
        let pts = to_points(&res);
        {
            let sp = &mut self.sheet_mut().shapes_new[a];
            sp.kind = "path".into();
            sp.points = pts;
            // 回転と反転は輪郭に焼き込んでいないので落とす(掛けたままだと
            // 二重に掛かる)。**落とすことは状態行で言う**
            sp.rot = 0.0;
            sp.flip_h = false;
            sp.flip_v = false;
        }
        // 引かれた側は消える(結合・交差・減算のどれでも1つになる)
        let keep = a - usize::from(b < a);
        self.sheet_mut().shapes_new.remove(b);
        self.shape_sel = Some(keep);
        self.shape_multi.clear();
        self.dirty = true;
        let name = match op {
            BoolOp::Union => ui::t!("union"),
            BoolOp::Intersect => ui::t!("intersect"),
            BoolOp::Subtract => ui::t!("subtract"),
        };
        self.status = ui::tf!(
            "done_turned_into_outline",
            name
        )
        .into();
    }
}
