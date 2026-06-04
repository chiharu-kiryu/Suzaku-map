package dev.suzaku.android.ime

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer

enum class SuzakuVoicePhase {
    IDLE,
    STARTING,
    LISTENING,
    HEARING,
    PROCESSING,
    READY,
    ERROR,
}

class SuzakuVoiceRecognizer(
    context: Context,
    private val onStatus: (String) -> Unit,
    private val onTranscript: (String) -> Unit,
    private val onPhaseChanged: (SuzakuVoicePhase) -> Unit,
) {
    private val recognizer =
        if (SpeechRecognizer.isRecognitionAvailable(context)) {
            SpeechRecognizer.createSpeechRecognizer(context)
        } else {
            null
        }

    private val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
        putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
        putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
        putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 3)
        putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, false)
    }

    var isListening: Boolean = false
        private set

    init {
        recognizer?.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {
                isListening = true
                onPhaseChanged(SuzakuVoicePhase.LISTENING)
                onStatus("Listening now")
            }

            override fun onBeginningOfSpeech() {
                onPhaseChanged(SuzakuVoicePhase.HEARING)
                onStatus("Hearing speech")
            }

            override fun onRmsChanged(rmsdB: Float) = Unit

            override fun onBufferReceived(buffer: ByteArray?) = Unit

            override fun onEndOfSpeech() {
                onPhaseChanged(SuzakuVoicePhase.PROCESSING)
                onStatus("Processing transcript")
            }

            override fun onError(error: Int) {
                isListening = false
                onPhaseChanged(SuzakuVoicePhase.ERROR)
                onStatus(
                    when (error) {
                        SpeechRecognizer.ERROR_AUDIO -> "Audio input error"
                        SpeechRecognizer.ERROR_CLIENT -> "Speech client stopped"
                        SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "Microphone permission required"
                        SpeechRecognizer.ERROR_NETWORK,
                        SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
                        -> "Speech network unavailable"
                        SpeechRecognizer.ERROR_NO_MATCH -> "No speech matched"
                        SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "Recognizer busy"
                        SpeechRecognizer.ERROR_SERVER -> "Speech service error"
                        SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "Listening timed out"
                        else -> "Speech unavailable"
                    }
                )
            }

            override fun onResults(results: Bundle?) {
                isListening = false
                val transcript = extractBestResult(results)
                if (transcript.isNotEmpty()) {
                    onTranscript(transcript)
                    onPhaseChanged(SuzakuVoicePhase.READY)
                    onStatus("Transcript ready")
                } else {
                    onPhaseChanged(SuzakuVoicePhase.ERROR)
                    onStatus("No speech matched")
                }
            }

            override fun onPartialResults(partialResults: Bundle?) {
                val transcript = extractBestResult(partialResults)
                if (transcript.isNotEmpty()) {
                    onTranscript(transcript)
                    onPhaseChanged(SuzakuVoicePhase.HEARING)
                    onStatus("Listening now")
                }
            }

            override fun onEvent(eventType: Int, params: Bundle?) = Unit
        })
    }

    fun isAvailable(): Boolean = recognizer != null

    fun start() {
        val instance = recognizer ?: run {
            onStatus("Speech recognizer unavailable")
            return
        }
        if (isListening) {
            return
        }
        onPhaseChanged(SuzakuVoicePhase.STARTING)
        onStatus("Starting microphone…")
        instance.startListening(intent)
    }

    fun stop() {
        if (!isListening) {
            return
        }
        recognizer?.stopListening()
        isListening = false
        onPhaseChanged(SuzakuVoicePhase.PROCESSING)
        onStatus("Stopping microphone")
    }

    fun destroy() {
        recognizer?.cancel()
        recognizer?.destroy()
        isListening = false
        onPhaseChanged(SuzakuVoicePhase.IDLE)
    }

    private fun extractBestResult(bundle: Bundle?): String {
        val matches = bundle
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            .orEmpty()
        return matches.firstOrNull().orEmpty().trim()
    }
}
