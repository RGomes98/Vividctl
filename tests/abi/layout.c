/*
 * ABI probe for the NVKMS ioctls vividctl uses.
 *
 * Compiled against the headers of one open-gpu-kernel-modules tag, it prints one `key=value` line per
 * size, offset and constant the Rust backend depends on. Diffing the output of two tags shows whether
 * the layout vividctl relies on changed between those driver versions.
 *
 * Build: see run.sh. The NVIDIA headers are MIT licensed (see the NOTICE file at the repository root).
 */
#include <stddef.h>
#include <stdio.h>
#include <sys/ioctl.h>

#include "nvtypes.h"
#include "nvkms-api.h"
#include "nvkms-ioctl.h"

#define SIZE(t)       printf("sizeof.%s=%zu\n", #t, sizeof(t))
#define OFFSET(t, f)  printf("offsetof.%s.%s=%zu\n", #t, #f, offsetof(t, f))
#define CONST(c)      printf("const.%s=%lld\n", #c, (long long)(c))

#define ALLOC_REQ  struct NvKmsAllocDeviceRequest
#define ALLOC_REP  struct NvKmsAllocDeviceReply
#define DISP_REP   struct NvKmsQueryDispReply
#define CONN_REP   struct NvKmsQueryConnectorStaticDataReply
#define DYN_REQ    struct NvKmsQueryDpyDynamicDataRequest
#define DYN_REP    struct NvKmsQueryDpyDynamicDataReply

int main(void) {
    CONST(NVKMS_IOCTL_IOWR);
    SIZE(struct NvKmsIoctlParams);

    CONST(NVKMS_IOCTL_ALLOC_DEVICE);
    CONST(NVKMS_IOCTL_FREE_DEVICE);
    CONST(NVKMS_IOCTL_QUERY_DISP);
    CONST(NVKMS_IOCTL_QUERY_CONNECTOR_STATIC_DATA);
    CONST(NVKMS_IOCTL_QUERY_DPY_DYNAMIC_DATA);
    CONST(NVKMS_IOCTL_SET_DPY_ATTRIBUTE);
    CONST(NVKMS_IOCTL_GET_DPY_ATTRIBUTE);
    CONST(NV_KMS_DPY_ATTRIBUTE_DIGITAL_VIBRANCE);

    CONST(NVKMS_NVIDIA_DRIVER_VERSION_STRING_LENGTH);
    CONST(NVKMS_MAX_SUBDEVICES);
    CONST(NVKMS_MAX_CONNECTORS_PER_DISP);
    CONST(NVKMS_DPY_NAME_SIZE);
    CONST(NVKMS_EDID_BUFFER_SIZE);
    CONST(NVKMS_ALLOC_DEVICE_STATUS_SUCCESS);
    CONST(NVKMS_ALLOC_DEVICE_STATUS_VERSION_MISMATCH);
    CONST(NVKMS_ALLOC_DEVICE_STATUS_NO_HARDWARE_AVAILABLE);

    CONST(NVKMS_CONNECTOR_TYPE_DP);
    CONST(NVKMS_CONNECTOR_TYPE_VGA);
    CONST(NVKMS_CONNECTOR_TYPE_DVI_I);
    CONST(NVKMS_CONNECTOR_TYPE_DVI_D);
    CONST(NVKMS_CONNECTOR_TYPE_ADC);
    CONST(NVKMS_CONNECTOR_TYPE_LVDS);
    CONST(NVKMS_CONNECTOR_TYPE_HDMI);

    /* AllocDevice */
    SIZE(struct NvKmsAllocDeviceParams);
    SIZE(ALLOC_REQ);
    SIZE(ALLOC_REP);
    OFFSET(struct NvKmsAllocDeviceParams, reply);
    OFFSET(ALLOC_REQ, versionString);
    OFFSET(ALLOC_REQ, deviceId);
    OFFSET(ALLOC_REQ, no3d);
    OFFSET(ALLOC_REP, status);
    OFFSET(ALLOC_REP, deviceHandle);
    OFFSET(ALLOC_REP, numDisps);
    OFFSET(ALLOC_REP, dispHandles);

    /* FreeDevice */
    SIZE(struct NvKmsFreeDeviceParams);
    OFFSET(struct NvKmsFreeDeviceRequest, deviceHandle);

    /* QueryDisp */
    SIZE(struct NvKmsQueryDispParams);
    OFFSET(struct NvKmsQueryDispParams, reply);
    OFFSET(struct NvKmsQueryDispRequest, deviceHandle);
    OFFSET(struct NvKmsQueryDispRequest, dispHandle);
    OFFSET(DISP_REP, numConnectors);
    OFFSET(DISP_REP, connectorHandles);
    OFFSET(DISP_REP, gpuString);

    /* QueryConnectorStaticData */
    SIZE(struct NvKmsQueryConnectorStaticDataParams);
    OFFSET(struct NvKmsQueryConnectorStaticDataParams, reply);
    OFFSET(struct NvKmsQueryConnectorStaticDataRequest, deviceHandle);
    OFFSET(struct NvKmsQueryConnectorStaticDataRequest, dispHandle);
    OFFSET(struct NvKmsQueryConnectorStaticDataRequest, connectorHandle);
    OFFSET(CONN_REP, dpyId);
    OFFSET(CONN_REP, type);
    OFFSET(CONN_REP, typeIndex);

    /* QueryDpyDynamicData */
    SIZE(struct NvKmsQueryDpyDynamicDataParams);
    SIZE(DYN_REQ);
    OFFSET(struct NvKmsQueryDpyDynamicDataParams, reply);
    OFFSET(DYN_REQ, deviceHandle);
    OFFSET(DYN_REQ, dispHandle);
    OFFSET(DYN_REQ, dpyId);
    OFFSET(DYN_REP, name);
    OFFSET(DYN_REP, connected);
    OFFSET(DYN_REP, edid.bufferSize);
    OFFSET(DYN_REP, edid.valid);
    OFFSET(DYN_REP, edid.buffer);

    /* SetDpyAttribute / GetDpyAttribute */
    SIZE(struct NvKmsSetDpyAttributeParams);
    OFFSET(struct NvKmsSetDpyAttributeRequest, deviceHandle);
    OFFSET(struct NvKmsSetDpyAttributeRequest, dispHandle);
    OFFSET(struct NvKmsSetDpyAttributeRequest, dpyId);
    OFFSET(struct NvKmsSetDpyAttributeRequest, attribute);
    OFFSET(struct NvKmsSetDpyAttributeRequest, value);
    SIZE(struct NvKmsGetDpyAttributeParams);
    OFFSET(struct NvKmsGetDpyAttributeRequest, deviceHandle);
    OFFSET(struct NvKmsGetDpyAttributeRequest, dispHandle);
    OFFSET(struct NvKmsGetDpyAttributeRequest, dpyId);
    OFFSET(struct NvKmsGetDpyAttributeRequest, attribute);
    OFFSET(struct NvKmsGetDpyAttributeParams, reply);
    OFFSET(struct NvKmsGetDpyAttributeReply, value);
    return 0;
}
