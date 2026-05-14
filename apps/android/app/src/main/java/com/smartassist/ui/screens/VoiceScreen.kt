package com.smartassist.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

@Composable
fun VoiceScreen(modifier: Modifier = Modifier) {
    var isListening by remember { mutableStateOf(false) }
    var transcript by remember { mutableStateOf("") }

    Column(
        modifier = modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "Voice Mode",
            style = MaterialTheme.typography.headlineLarge
        )

        Text(
            text = transcript,
            modifier = Modifier.padding(16.dp),
            style = MaterialTheme.typography.bodyLarge
        )

        Box(
            modifier = Modifier
                .size(120.dp)
                .background(
                    if (isListening) Color.Red.copy(alpha = 0.2f)
                    else MaterialTheme.colorScheme.primary.copy(alpha = 0.2f),
                    CircleShape
                ),
            contentAlignment = Alignment.Center
        ) {
            IconButton(
                onClick = { isListening = !isListening },
                modifier = Modifier.size(64.dp)
            ) {
                Icon(
                    painter = painterResource(id = android.R.drawable.ic_btn_speak_now),
                    contentDescription = if (isListening) "Stop" else "Listen",
                    modifier = Modifier.size(48.dp),
                    tint = if (isListening) Color.Red
                    else MaterialTheme.colorScheme.primary
                )
            }
        }

        Text(
            text = if (isListening) "Listening..." else "Tap to speak",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
