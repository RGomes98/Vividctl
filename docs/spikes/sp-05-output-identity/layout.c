#include <stddef.h>
#include <stdio.h>
#include "nvtypes.h"
#include "nvkms-api.h"
#define O(f) printf("reply.%s = %zu\n", #f, offsetof(struct NvKmsQueryDpyDynamicDataReply, f))
int main(void) {
    printf("NVKMS_EDID_INFO_STRING_LENGTH = %d\n", NVKMS_EDID_INFO_STRING_LENGTH);
    O(name); O(connected); O(physicalDimensions); O(dp.guid.valid); O(dp.guid.buffer); O(dp.guid.str);
    O(edid.bufferSize); O(edid.valid); O(edid.buffer); O(edid.infoString);
    printf("sizeof reply = %zu\n", sizeof(struct NvKmsQueryDpyDynamicDataReply));
    return 0;
}
