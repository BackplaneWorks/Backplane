package dev.backplane.mobile

import androidx.compose.runtime.staticCompositionLocalOf

// The resolved tag from Screen.locale. No native locale policy.
val LocalBackplaneLanguage = staticCompositionLocalOf { "en" }
