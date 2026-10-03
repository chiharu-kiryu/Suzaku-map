"""Bounded, read-only X11 focus ancestry without a display dependency."""


def focus_ancestry(current, parent_for, *, max_depth=16):
    """Return only the focus XID and ancestors proved by ``parent_for``.

    X11's None (0) and PointerRoot (1) are not owned windows. A missing or
    failed query, invalid parent, cycle or depth limit stops traversal: no
    unknown edge can establish ownership. An already-proved ancestor remains
    usable even when the tree above it cannot be read. Callers must match the
    exact owned window, never just a shared root, and reject their real root
    window as an ownership target.
    """
    if type(max_depth) is not int or max_depth <= 0:
        raise ValueError("focus ancestry depth must be a positive integer")
    chain = []
    while type(current) is int and current > 1 and current not in chain:
        chain.append(current)
        if len(chain) >= max_depth:
            break
        try:
            current = parent_for(current)
        except Exception:
            break  # A disappearing window or failed query cannot add an edge.
    return tuple(chain)
