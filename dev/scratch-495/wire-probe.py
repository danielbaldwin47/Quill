# wire-probe.py: puts the #495 probe marks into the scratch worktree (not for merge).
W = '/home/diggle/repos/quill/.claude/worktrees/probe-495/'
def rep(p, a, b):
    s = open(W + p).read(); assert s.count(a) == 1, (p, a); open(W + p, 'w').write(s.replace(a, b))
rep('quill/src/main.rs', "mod preview;\n", "mod preview;\nmod probe;\n")
rep('quill/src/harness.rs', """    let widget: gtk::Widget = window.as_ref().clone();
""", """    let widget: gtk::Widget = window.as_ref().clone();
    crate::probe::start(&widget);
""")
rep('quill/src/harness.rs', """                println!("{}", launch_settled_line(at, from_exec));""", """                println!("{}", launch_settled_line(at, from_exec));
                crate::probe::line("SETTLED");""")
rep('quill/src/window.rs', """                    if window.imp().editor.drain_syntax(&window.document()) {""", """                    crate::probe::line("drain+");
                    let more = window.imp().editor.drain_syntax(&window.document());
                    crate::probe::line("drain-");
                    if more {""")
rep('quill/src/window.rs', """        let document = self.document();
        self.imp().bars.set_count(document.text());
    }""", """        crate::probe::line("recount+");
        let document = self.document();
        self.imp().bars.set_count(document.text());
        crate::probe::line("recount-");
    }""")
rep('quill/src/window.rs', """                    window.imp().syntax_wake.take();
                    window.imp().editor.submit_syntax(&window.document());""", """                    window.imp().syntax_wake.take();
                    crate::probe::line("submit+");
                    window.imp().editor.submit_syntax(&window.document());
                    crate::probe::line("submit-");""")
rep('tools/bench.mjs', """  const ours = await stage.launch(path.join(root, BINARY), launchArgv(root, out, regime));""", """  if (process.env.BENCH_PROBE_DIR) process.env.QUILL_PROBE = path.join(process.env.BENCH_PROBE_DIR, `probe-${regime.name}.txt`);
  const ours = await stage.launch(path.join(root, BINARY), launchArgv(root, out, regime));""")
rep('tools/bench.mjs', """  } finally {
    stage.kill(ours.child);
  }""", """  } finally {
    stage.kill(ours.child);
    if (process.env.BENCH_PROBE_DIR) fs.copyFileSync(out, path.join(process.env.BENCH_PROBE_DIR, `capture-${regime.name}.jsonl`));
  }""")
