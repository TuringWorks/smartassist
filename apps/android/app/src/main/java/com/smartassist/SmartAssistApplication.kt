package com.smartassist

import android.app.Application
import com.smartassist.data.DeviceStore
import com.smartassist.network.GatewayClient
import com.smartassist.voice.VoiceService

class SmartAssistApplication : Application() {
    companion object {
        lateinit var instance: SmartAssistApplication
            private set
    }

    val gatewayClient = GatewayClient()
    val deviceStore = DeviceStore()
    val voiceService = VoiceService()

    override fun onCreate() {
        super.onCreate()
        instance = this
    }
}
