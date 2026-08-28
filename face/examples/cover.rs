fn main() {
    for (name, tabs) in [("writer", face::ribbon::writer_tabs()),
                         ("calc", face::ribbon::calc_tabs())] {
        let (ready, all) = face::ribbon::progress(tabs);
        println!("{name:8} {ready:4} / {all:4}  ({:.0}%)", ready as f32 * 100.0 / all as f32);
        // 段ごと
        for t in tabs {
            let r = t.cmds.iter().filter(|c| c.ready).count();
            if r < t.cmds.len() {
                println!("    {:14} {r:3} / {:3}", t.name, t.cmds.len());
            }
        }
    }
}
