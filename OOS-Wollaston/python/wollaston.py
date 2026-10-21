"""Standard-library Python binding for Wollaston C ABI 1. No UI dependency."""
import ctypes
import json
from pathlib import Path

class Wollaston:
    def __init__(self, library):
        self.lib = ctypes.CDLL(str(Path(library).resolve()))
        self.lib.wollaston_abi_version.restype = ctypes.c_uint32
        if self.lib.wollaston_abi_version() != 1:
            raise RuntimeError('Unsupported Wollaston ABI')
        self.lib.wollaston_execute.argtypes = [ctypes.c_char_p, ctypes.c_void_p]
        self.lib.wollaston_execute.restype = ctypes.c_void_p
        self.lib.wollaston_free.argtypes = [ctypes.c_void_p]
        self.lib.wollaston_free.restype = None
        self.lib.wollaston_cancel_new.restype = ctypes.c_void_p
        for name in ('wollaston_cancel', 'wollaston_cancel_free'):
            getattr(self.lib, name).argtypes = [ctypes.c_void_p]
            getattr(self.lib, name).restype = None

    def execute(self, request, token=None):
        """Synchronous independent request. ctypes releases the GIL during compute."""
        raw = json.dumps(request, allow_nan=False).encode('utf-8')
        if len(raw) > 4_000_000:
            raise ValueError('request exceeds 4 MB')
        if token is not None and (token.engine is not self or token.closed):
            raise ValueError('Cancellation token belongs to another engine or is closed')
        result = self.lib.wollaston_execute(raw, token.pointer if token else None)
        try:
            envelope = json.loads(ctypes.string_at(result))
        finally:
            self.lib.wollaston_free(result)
        if not envelope['ok']:
            raise RuntimeError(envelope['error'])
        return envelope['result']

    def cancellation(self):
        return Cancellation(self)

class Cancellation:
    """One-shot token. Close only after every executing worker has joined."""
    def __init__(self, engine):
        self.engine = engine
        self.pointer = engine.lib.wollaston_cancel_new()
        self.closed = False

    def cancel(self):
        if self.closed:
            raise RuntimeError('Token already closed')
        self.engine.lib.wollaston_cancel(self.pointer)

    def close(self):
        if not self.closed:
            self.engine.lib.wollaston_cancel_free(self.pointer)
            self.closed = True

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
