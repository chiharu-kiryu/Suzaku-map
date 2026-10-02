"""Read-only GTK editor observation for owned, isolated application QA."""
from dataclasses import dataclass


def editor_environment(base, root, run):
    """Keep the fixture editor's background draft saver outside its 240s run."""
    assert base.get("XDG_CONFIG_HOME") == str(root / "config"), "editor config is not isolated"
    env = dict(base)
    env.update(NO_AT_BRIDGE="0", GTK_A11Y="atspi", GSETTINGS_BACKEND="keyfile",
               XDG_CONFIG_DIRS=str(root / "editor-config") + ":" +
               (base.get("XDG_CONFIG_DIRS") or "/etc/xdg"))
    # Preserve the shared private XDG_CONFIG_HOME: IBus watches its bus discovery
    # file there for reconnects. Only GTK defaults use the editor-only directory.
    # Ctrl+S can land during the editor's own periodic draft save, when its
    # Save action is disabled. These are input tests, not editor autosave tests.
    # Only this disposable config changes; the host/other apps keep memory settings.
    command = ["gsettings", "set", "org.gnome.TextEditor", "auto-save-delay", "300"]
    run(command, env=env, check=True, capture_output=True, text=True, timeout=3)
    result = run(["gsettings", "get", "org.gnome.TextEditor", "auto-save-delay"],
                 env=env, check=True, capture_output=True, text=True, timeout=3)
    assert result.stdout.strip() == "uint32 300", "owned editor autosave setting was not applied"
    return env


@dataclass(frozen=True)
class EditorState:
    text: str
    busy: bool

    def saved_text(self):
        # GtkSourceView serializes an implicit final newline without adding it
        # to the buffer. Do not strip spaces or accept partial/normalized text.
        return self.text + "\n" if self.text and not self.text.endswith("\n") else self.text


def nodes(root, children=None):
    # Optional topology reuse is scoped to ONE observation, never across input
    # or saves. AT-SPI queries are synchronous D-Bus calls, not cheap fields.
    if children is None:
        children = {}
    pending = [root]
    seen = set()
    while pending:
        node = pending.pop()
        if node is None or node in seen:
            # AT-SPI can expose the same object twice during cache updates.
            # Deduplicate by object identity, not role/name/content.
            continue
        seen.add(node)
        assert len(seen) <= 1000, "unexpectedly large owned accessibility tree"
        yield node
        if node not in children:
            children[node] = [node.get_child_at_index(i) for i in range(node.get_child_count())]
        pending.extend(children[node])


def visible_nodes(root, showing):
    # A restarted GTK3 popup may have fresh widgets but stale cached states.
    # Invalidate only the owned application tree, never the whole desktop.
    root.clear_cache()
    return [node for node in nodes(root) if node.get_state_set().contains(showing)]


class GtkEditorObserver:
    def __init__(self, atspi, process, path):
        self.atspi, self.process, self.path = atspi, process, path

    def state(self):
        assert self.process.poll() is None, "owned editor exited unexpectedly"
        api = self.atspi
        desktop = api.get_desktop(0)
        apps = [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
        apps = [app for app in apps if app is not None and app.get_process_id() == self.process.pid]
        assert len(apps) <= 1, "ambiguous owned editor process"
        if not apps:
            return None
        # State/child caches can lag the actual save indicator or focus. Query
        # the owned application's current state, not last-delivered cache data.
        apps[0].set_cache_mask(api.Cache.NONE)
        children = {}
        roles = {node: node.get_role() for node in nodes(apps[0], children)}
        frames = [node for node, role in roles.items()
                  if role == api.Role.FRAME and self.path.name in node.get_name()]
        assert len(frames) <= 1, "ambiguous owned editor document"
        if not frames:
            return None
        states = {node: node.get_state_set() for node in nodes(frames[0], children)}
        visible = {node: state for node, state in states.items() if state.contains(api.StateType.SHOWING)}
        texts = [node for node, state in visible.items() if roles[node] == api.Role.TEXT and
                 state.contains(api.StateType.MULTI_LINE) and state.contains(api.StateType.FOCUSED)]
        # Only the currently focused multiline editor is the physical key target.
        assert len(texts) <= 1, "ambiguous owned editor text view"
        if not texts:
            return None
        # The editor keeps its progress indicator visible until its asynchronous
        # load/save finishes (including metadata), then fades it out. File bytes
        # alone can appear earlier, while the Save action is still disabled.
        busy = any(roles[node] == api.Role.PROGRESS_BAR for node in visible)
        # Accessible.get_text() is the deprecated interface getter in PyGObject;
        # call the Text interface explicitly to read its contents.
        return EditorState(api.Text.get_text(texts[0], 0, -1), busy)


def save_observed_document(observer, path, expected, send_save, wait):
    """One physical save, after exact buffer/idle checks; never retry input."""
    observed = None

    def ready():
        nonlocal observed
        observed = observer.state()
        return observed is not None and not observed.busy and observed.saved_text() == expected

    try:
        wait(ready, "owned editor buffer or idle state differs before save")
        send_save()
        wait(lambda: ready() and path.read_text() == expected,
             "owned editor buffer, idle state or saved document differs after save")
    except AssertionError as error:
        # Only the registered fixture document/buffer, never personal desktop text.
        raise AssertionError(f"{error}: {path.name}, expected={expected!r}, "
                             f"observed={observed!r}, saved={path.read_text()!r}") from error
