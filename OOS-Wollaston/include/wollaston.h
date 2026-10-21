#ifndef OOS_WOLLASTON_H
#define OOS_WOLLASTON_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* ABI 1, cdecl, UTF-8 JSON. No caller allocation crosses the ABI boundary.
   execute is synchronous and thread-safe for independent requests.
   Cancel tokens are one-shot. Keep alive until every using execute returns.
   Input is a readable NUL-terminated string (maximum 4 MB excluding NUL).
   Output is {"ok":true,"result":...} or {"ok":false,"error":"..."}.
   Do not use free() on output. Never free a token while another thread uses it. */
uint32_t wollaston_abi_version(void);
void *wollaston_cancel_new(void);
void wollaston_cancel(const void *token);
void wollaston_cancel_free(void *token);
char *wollaston_execute(const char *request, const void *token);
void wollaston_free(char *result);
#ifdef __cplusplus
}
#endif
#endif
