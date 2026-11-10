// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit.example

import android.app.Activity
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.WindowInsets
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import nl.mattix.walkit.LayeredSkinFiles
import nl.mattix.walkit.WalArchive
import nl.mattix.walkit.WithGeneratedImages
import nl.mattix.walkit.ZipSkinFiles
import java.io.File
import java.io.InputStream

/**
 * Two buttons and a [SkinView]. "Open skin" picks a `.wsz` (classic) or `.wal` (modern) skin;
 * "Default set" picks a zip of Winamp's wasabi folder, which modern skins draw lists,
 * scrollbars and frames from. Both are copied into the app's files and the skin opened last
 * opens again on start.
 */
class MainActivity : Activity() {
    private lateinit var skinView: SkinView
    private lateinit var status: TextView

    private val skinDir get() = File(filesDir, "skin")
    private val defaultSet get() = File(filesDir, "default-set.zip")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        skinView = SkinView(this)
        status = TextView(this).apply { setTextColor(Color.LTGRAY) }
        val buttons =
            LinearLayout(this).apply {
                addView(button("Open skin") { pick(PICK_SKIN) })
                addView(button("Default set") { pick(PICK_DEFAULT_SET) })
            }
        val root =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setBackgroundColor(Color.rgb(24, 24, 28))
                addView(buttons)
                addView(status)
                addView(skinView, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            }
        // the app draws behind the system bars; keep the content clear of them
        root.setOnApplyWindowInsetsListener { v, insets ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                val bars = insets.getInsets(WindowInsets.Type.systemBars())
                v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            } else {
                @Suppress("DEPRECATION")
                v.setPadding(insets.systemWindowInsetLeft, insets.systemWindowInsetTop, insets.systemWindowInsetRight, insets.systemWindowInsetBottom)
            }
            insets
        }
        setContentView(root)
        importForTesting(intent)
        openSkin()
    }

    override fun onDestroy() {
        skinView.close()
        super.onDestroy()
    }

    private fun button(
        label: String,
        onClick: () -> Unit,
    ) = Button(this).apply {
        text = label
        setOnClickListener { onClick() }
    }

    // ---- picking files -----------------------------------------------------------------------

    private fun pick(requestCode: Int) {
        val intent =
            Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                // .wsz and .wal have no MIME type of their own
                type = "*/*"
            }
        @Suppress("DEPRECATION")
        startActivityForResult(intent, requestCode)
    }

    @Deprecated("Activity's own result callback; this example uses no AndroidX")
    override fun onActivityResult(
        requestCode: Int,
        resultCode: Int,
        data: Intent?,
    ) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data ?: return
        if (resultCode != RESULT_OK) return
        val opened = contentResolver.openInputStream(uri) ?: return
        when (requestCode) {
            PICK_SKIN -> opened.use { keepSkin(displayName(uri), it) }
            PICK_DEFAULT_SET -> opened.use { keepDefaultSet(it) }
        }
        openSkin()
    }

    private fun displayName(uri: Uri): String =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: "skin.wsz"

    /** Keeps one skin: the folder holds the last one picked, under its own name. */
    private fun keepSkin(
        name: String,
        input: InputStream,
    ) {
        skinDir.deleteRecursively()
        skinDir.mkdirs()
        File(skinDir, File(name).name).outputStream().use { input.copyTo(it) }
    }

    private fun keepDefaultSet(input: InputStream) {
        defaultSet.outputStream().use { input.copyTo(it) }
    }

    /**
     * For a check without the picker, in a debug build only: `--es skin <name>` and
     * `--es defaults <name>` on `am start` import those files from the app's external files
     * folder (`/sdcard/Android/data/nl.mattix.walkit.example/files`, made on the first start) as
     * if picked. The app reads that folder without a permission, and `adb push` writes to it.
     */
    private fun importForTesting(intent: Intent) {
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0) return
        val dir = getExternalFilesDir(null) ?: return
        intent.getStringExtra("defaults")?.let { name -> File(dir, name).inputStream().use { keepDefaultSet(it) } }
        intent.getStringExtra("skin")?.let { name -> File(dir, name).inputStream().use { keepSkin(name, it) } }
    }

    // ---- opening the skin --------------------------------------------------------------------

    private fun openSkin() {
        val file = skinDir.listFiles()?.firstOrNull()
        if (file == null) {
            status.text = "Pick a .wsz or .wal skin."
            return
        }
        try {
            val skin = ZipSkinFiles(file)
            val modern = WalArchive.isModern(skin.names())
            val haveDefaults = defaultSet.exists()
            // a modern skin reads Winamp's default set under paths starting with wasabi-base/
            val files = if (haveDefaults) LayeredSkinFiles(skin, ZipSkinFiles(defaultSet)) else skin
            skinView.show(WithGeneratedImages(files), file.nameWithoutExtension)
            status.text =
                when {
                    modern && !haveDefaults -> "${file.name}: a modern skin; pick a default set for its lists and frames."
                    else -> file.name
                }
        } catch (e: Exception) {
            Log.w("walkit", "cannot open ${file.name}", e)
            status.text = "${file.name}: ${e.message}"
        }
    }

    private companion object {
        const val PICK_SKIN = 1
        const val PICK_DEFAULT_SET = 2
    }
}
