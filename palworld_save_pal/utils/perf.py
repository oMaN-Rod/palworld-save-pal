import contextlib
import gc


@contextlib.contextmanager
def gc_paused():
    """Disable cyclic GC for the duration of a hot build-up phase.

    GVAS read/write builds large tree-shaped dicts and lists with no reference
    cycles, so generational collections during the build are pure overhead.
    """
    was_enabled = gc.isenabled()
    if was_enabled:
        gc.disable()
    try:
        yield
    finally:
        if was_enabled:
            gc.enable()
            gc.collect()
