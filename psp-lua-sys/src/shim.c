#include <stddef.h>
#include "lua.h"

/*
** Bridges Lua's calling convention to a Rust host function without putting a
** Rust frame between lua_error and the setjmp it unwinds to: on Windows,
** longjmp runs a real SEH unwind through every intervening frame, and a Rust
** frame that has touched the allocator carries cleanup metadata that unwind
** invokes, corrupting the heap. Rust returns first; this frame raises.
**
** dispatch returns the result count, or negative to signal an error via the
** message/len out-parameters. A non-NULL message is owned by Rust and must
** be handed back to free_message once copied into Lua.
*/
typedef int (*psp_dispatch_fn)(lua_State *L, void *host_fn,
                               const char **message, size_t *len,
                               void **owner);
typedef void (*psp_free_fn)(void *owner);

int psp_host_trampoline(lua_State *L) {
  psp_dispatch_fn dispatch = (psp_dispatch_fn)lua_touserdata(L, lua_upvalueindex(1));
  void *host_fn = lua_touserdata(L, lua_upvalueindex(2));
  psp_free_fn free_message = (psp_free_fn)lua_touserdata(L, lua_upvalueindex(3));

  const char *message = NULL;
  size_t len = 0;
  void *owner = NULL;

  int n = dispatch(L, host_fn, &message, &len, &owner);
  if (n >= 0) {
    return n;
  }

  /* lua_pushlstring may itself raise under the memory cap. If it does, `owner`
  ** leaks -- one error message, bounded, and far better than the alternative. */
  lua_pushlstring(L, message ? message : "host error", len);
  if (owner != NULL) {
    free_message(owner);
  }
  return lua_error(L);
}
