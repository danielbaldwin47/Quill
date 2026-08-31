# Quill uses plain GTK4, not libadwaita

*Reread under [ADR 0015](0015-the-design-oracle-outranks-the-parity-oracle.md): chrome is outside the
Design oracle's reach, so "judged blind against iA Writer" below is the JavaScript app's history —
the native chrome is judged against the Parity oracle. The decision stands.*

The application is built on GTK4 alone, styled by Quill's own CSS, with libadwaita neither linked nor
depended on. Quill's chrome is iA-styled and judged blind against iA Writer; libadwaita's stylesheet
and widgets make every window look like GNOME by default and win arguments with custom CSS. What
libadwaita would give for free (following the system's dark mode) comes from the settings portal
through GTK itself. Settled in [#11](https://github.com/danielbaldwin47/Quill/issues/11).

A single libadwaita widget wanted later can be adopted without reversing this: the decision is that
the default look is Quill's, not that the library is forbidden.
