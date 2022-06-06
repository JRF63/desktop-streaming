package com.debug.myapplication

import android.os.Bundle
import android.view.Surface
import android.view.SurfaceHolder
import androidx.appcompat.app.AppCompatActivity
import androidx.constraintlayout.widget.ConstraintSet
import com.debug.myapplication.databinding.ActivityMainBinding

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    private val layoutConstraints: ConstraintSet = ConstraintSet()
    private var nativeInstance: ULong = 0u

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        layoutConstraints.clone(binding.root)

        //nativeInstance = createNativeInstance(nativeInstance)

        binding.surfaceView.holder.addCallback(object: SurfaceHolder.Callback {
            override fun surfaceCreated(p0: SurfaceHolder) {
                nativeInstance = createNativeInstance(nativeInstance)
                sendSurfaceChanged(nativeInstance, p0.surface)
            }

            override fun surfaceChanged(holder: SurfaceHolder, p1: Int, p2: Int, p3: Int) {
                //sendSurfaceChanged(nativeInstance, holder.surface)
            }

            override fun surfaceDestroyed(p0: SurfaceHolder) {
                sendSurfaceDestroyed(nativeInstance)
            }
        })
    }

    override fun onDestroy() {
        super.onDestroy()
        sendDestroySignal(nativeInstance)
    }

    private fun setSurfaceViewAspectRatio(aspectRatio: String) {
        this@MainActivity.runOnUiThread {
            layoutConstraints.setDimensionRatio(binding.surfaceView.id, aspectRatio)
            layoutConstraints.applyTo(binding.root)
        }
    }

    // Single letter function names for obfuscation and easier interfacing in
    // native code - this prevents appending random characters to the function
    // signatures (i.e., createNativeInstance => createNativeInstance-V0uzKk8)

    @JvmName("a")
    private external fun createNativeInstance(nativeInstance: ULong): ULong
    @JvmName("b")
    private external fun sendDestroySignal(nativeInstance: ULong)
    @JvmName("c")
    private external fun sendSurfaceChanged(nativeInstance: ULong, surface: Surface)
    @JvmName("d")
    private external fun sendSurfaceDestroyed(nativeInstance: ULong)

    companion object {
        init {
            System.loadLibrary("client_android")
        }
    }
}