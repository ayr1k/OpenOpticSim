#include "wollaston.h"
#include <stdio.h>
int main(void) {
    char *result = wollaston_execute("{}", NULL);
    printf("ABI %u: %s\n", wollaston_abi_version(), result);
    wollaston_free(result);
    return wollaston_abi_version() == 1 ? 0 : 1;
}
