package com.smartassist.data

class DeviceStore {
    private val devices = mutableListOf<Device>()

    fun allDevices(): List<Device> = devices.toList()

    fun add(device: Device) {
        devices.add(device)
    }

    fun remove(id: String) {
        devices.removeAll { it.id == id }
    }

    fun updateConnection(id: String, connected: Boolean) {
        val index = devices.indexOfFirst { it.id == id }
        if (index != -1) {
            devices[index] = devices[index].copy(connected = connected)
        }
    }
}
