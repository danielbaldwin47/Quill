set pagination off
set print thread-events off
set breakpoint pending on
set startup-with-shell off
set debuginfod enabled off
python
import gdb, time
START = time.monotonic()
class Op(gdb.Breakpoint):
    def stop(self):
        try:
            self.say()
        except Exception as e:
            print("=== set_opacity at %.3f s: (%s)" % (time.monotonic() - START, e))
        return False
    def say(self):
        t = time.monotonic() - START
        w = gdb.parse_and_eval("$rdi")
        name = gdb.parse_and_eval("(char*)g_type_name(**(unsigned long**)%d)" % int(w)).string()
        parent = gdb.parse_and_eval("(void*)gtk_widget_get_parent(%d)" % int(w))
        pname = gdb.parse_and_eval("(char*)g_type_name(**(unsigned long**)%d)" % int(parent)).string() if int(parent) else "-"
        op = float(gdb.parse_and_eval("$xmm0.v2_double[0]"))
        print("=== set_opacity at %.3f s: %s (parent %s) -> %.2f" % (t, name, pname, op))
        return False
Op("gtk_widget_set_opacity")
end
run
