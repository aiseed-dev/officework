//! **図形と画像、検索と絞り込み。** 盤面の上に載るもの。

use crate::*;

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
    pub(crate) fn image_drag_at(&mut self, x: f32, y: f32) {
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
            self.status = format!("大きさ: {w:.0}×{h:.0}px").into();
        } else {
            let (nx, ny) = (ox + x - gx, oy + y - gy);
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
                        self.status = format!("画像を {} に留めました", at.a1()).into();
                    }
                }
            }
        }
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
        self.status = ui::t!("画像を削除しました(Ctrl+Z で戻せます)").into();
        true
    }

    /// 図形の右クリックメニューの実体(切り貼り・重なり順・回転・SVG保存)。
    /// window を要らなくしてあるので試験からそのまま呼べる
    pub(crate) fn shape_menu_action(&mut self, id: &str) {
        match id {
            "sh-copy" | "sh-cut" => {
                let Some(i) = self.shape_sel else { return };
                let Some(sp) = self.sheet().shapes_new.get(i).cloned() else { return };
                self.shape_clip = Some(sp);
                if id == "sh-cut" {
                    self.checkpoint();
                    self.sheet_mut().shapes_new.remove(i);
                    self.shape_sel = None;
                    self.shape_multi.clear();
                    self.dirty = true;
                    self.status = ui::t!("図形を切り取りました(貼り付けで戻せます)").into();
                } else {
                    self.status = ui::t!("図形をコピーしました").into();
                }
            }
            "sh-paste" => {
                let Some(mut sp) = self.shape_clip.clone() else {
                    self.status = ui::t!("貼り付ける図形がありません(先に図形をコピー)").into();
                    return;
                };
                self.checkpoint();
                sp.at = self.cursor;
                (sp.dx_px, sp.dy_px) = (4.0, 4.0);
                self.sheet_mut().shapes_new.push(sp);
                self.shape_sel = Some(self.sheet().shapes_new.len() - 1);
                self.dirty = true;
                self.status = ui::tf!("図形を {} に貼り付けました", self.cursor.a1()).into();
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
                    ui::t!("図形を削除しました(Ctrl+Z で戻せます)").into()
                } else {
                    ui::tf!("{} 個の図形を削除しました(Ctrl+Z で戻せます)", idx.len()).into()
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
                    self.status = ui::t!("もうその位置です(後に描く図形が前に出ます)").into();
                    return;
                }
                self.checkpoint();
                let sp = self.sheet_mut().shapes_new.remove(i);
                self.sheet_mut().shapes_new.insert(j, sp);
                self.shape_sel = Some(j);
                self.dirty = true;
                self.status = match id {
                    "sh-front" => ui::t!("最前面へ移動しました").into(),
                    "sh-forward" => ui::t!("前面へ移動しました").into(),
                    "sh-backward" => ui::t!("背面へ移動しました").into(),
                    _ => ui::t!("最背面へ移動しました").into(),
                };
            }
            "sh-rot-r" | "sh-rot-l" => {
                let d = if id == "sh-rot-r" { 90.0 } else { -90.0 };
                self.shape_edit(move |sp| sp.rot = (sp.rot + d).rem_euclid(360.0));
                self.status = ui::t!("90度回しました").into();
            }
            "sh-flip-h" => {
                self.shape_edit(|sp| sp.flip_h = !sp.flip_h);
                self.status = ui::t!("左右に反転しました").into();
            }
            "sh-flip-v" => {
                self.shape_edit(|sp| sp.flip_v = !sp.flip_v);
                self.status = ui::t!("上下に反転しました").into();
            }
            // 画像として保存 = SVG(うちの図形の素の姿。嘘の PNG 変換はしない)
            "sh-save" => {
                let Some(i) = self.shape_sel else { return };
                let Some(sp) = self.sheet().shapes_new.get(i) else { return };
                let svg = sp.to_svg();
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("figure.svg")
                    .save_file()
                else {
                    self.status = ui::t!("保存をやめました").into();
                    return;
                };
                self.status = match std::fs::write(&path, svg) {
                    Ok(_) => ui::tf!("SVG で保存しました: {}", path.display().to_string()).into(),
                    Err(e) => ui::tf!("保存できません: {}", e.to_string()).into(),
                };
            }
            // 詳細設定 = 右の設定パネル(選択中はいつも出ている)
            "sh-settings" => {
                self.status = ui::t!("設定は右の「図形の設定」のパネルでどうぞ").into();
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
                "{} 個以上の図形を選んでから(Ctrl+クリックで足せます)",
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
            "sh-al-l" => ui::tf!("{} 個を左に揃えました", n).into(),
            "sh-al-c" => ui::tf!("{} 個を左右の中央に揃えました", n).into(),
            "sh-al-r" => ui::tf!("{} 個を右に揃えました", n).into(),
            "sh-al-t" => ui::tf!("{} 個を上に揃えました", n).into(),
            "sh-al-m" => ui::tf!("{} 個を上下の中央に揃えました", n).into(),
            "sh-al-b" => ui::tf!("{} 個を下に揃えました", n).into(),
            "sh-dist-h" => ui::tf!("{} 個を横に等間隔で並べました", n).into(),
            _ => ui::tf!("{} 個を縦に等間隔で並べました", n).into(),
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
            self.status = ui::tf!("回転: {}度", format!("{deg:.0}")).into();
        }
    }

    /// 図形のドラッグ(移動 or 右下の掴みで大きさ変更)。
    pub(crate) fn shape_drag_at(&mut self, x: f32, y: f32) {
        let Some((i, (gx, gy), (ox, oy), resize)) = self.shape_drag else { return };
        if self.sheet().shapes_new.len() <= i {
            return;
        }
        if resize {
            let sp = &mut self.sheet_mut().shapes_new[i];
            sp.width_px = (x - ox).max(16.0);
            sp.height_px = (y - oy).max(16.0);
            let (w, h) = (sp.width_px, sp.height_px);
            self.dirty = true;
            self.status = format!("大きさ: {w:.0}×{h:.0}px").into();
        } else {
            // 移動: 掴んだときのずれを保って、左上の来るセルに留め直す。
            // セルからのはみ出しは px のずらしとして持つ(位置が飛ばない)
            let (nx, ny) = (ox + x - gx, oy + y - gy);
            if let (Some(c), Some(r)) = (self.col_at(nx.max(HEAD_W)), self.row_at(ny.max(ROW_H))) {
                let at = Pos::new(r, c);
                if let Some((cx0, cy0)) = self.cell_origin_px(at) {
                    let (dx, dy) = ((nx - cx0).max(0.0), (ny - cy0).max(0.0));
                    let sp = &mut self.sheet_mut().shapes_new[i];
                    if sp.at != at || (sp.dx_px - dx).abs() > 0.5 || (sp.dy_px - dy).abs() > 0.5 {
                        sp.at = at;
                        sp.dx_px = dx;
                        sp.dy_px = dy;
                        self.dirty = true;
                        self.status = format!("図形を {} に留めました", at.a1()).into();
                    }
                }
            }
        }
    }

    /// 「次を検索」。いまのセルの次(行→列の順)から探し、末尾まで行ったら
    /// 頭に戻る。式の中の文字も探す(editable = 打った通りの姿)。
    pub(crate) fn find_next(&mut self, term: &str) {
        let hits: Vec<Pos> = self
            .sheet()
            .cells
            .iter()
            .filter(|(_, c)| c.editable().contains(term) || c.value.display().contains(term))
            .map(|(p, _)| *p)
            .collect();
        if hits.is_empty() {
            self.status = format!("「{term}」は見つかりません").into();
            return;
        }
        let cur = self.cursor;
        let next = hits.iter().find(|p| **p > cur).copied().unwrap_or(hits[0]);
        self.anchor = None;
        self.cursor = next;
        self.follow();
        self.sync_input();
        self.status = format!(
            "「{term}」: {}({} カ所)。もう一度「置き換え」で次へ",
            next.a1(),
            hits.len()
        )
        .into();
        // 次回のパネルの初期値に残す(続けて探すのが検索の常)
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
