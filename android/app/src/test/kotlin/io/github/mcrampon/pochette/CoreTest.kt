package io.github.mcrampon.pochette

import io.github.mcrampon.pochette.core.Config
import io.github.mcrampon.pochette.core.LibraryView
import io.github.mcrampon.pochette.core.Period
import io.github.mcrampon.pochette.core.Pochette
import io.github.mcrampon.pochette.core.PochetteException
import io.github.mcrampon.pochette.core.Sort
import io.github.mcrampon.pochette.core.trackDuration
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.nio.file.Files

/** The core, from Kotlin, on this computer's JVM, through the generated bindings. */
class CoreTest {
    @Test
    fun formats() {
        assertEquals("3:33", trackDuration(213u))
        assertEquals("1:02:05", trackDuration(3725u))
    }

    @Test
    fun startsSignedOut() {
        val dir = Files.createTempDirectory("pochette-test").toFile()
        try {
            val core = Pochette(Config(dir.resolve("plays").path, dir.resolve("sync").path, "Test"))
            assertNull(core.account())
            assertNull(core.session())
            assertTrue(core.library(LibraryView("", emptyList(), emptyList(), Sort.DATE_ADDED, false)).isEmpty())
            assertEquals(0uL, core.stats(Period.All).time)
            assertEquals("Test", core.syncStatus().name)
            // A suspend call, on the core's runtime: a session it can't read.
            runBlocking {
                try {
                    core.restore("not a session")
                    fail("restored a session from nothing")
                } catch (expected: PochetteException.SignedOut) {
                    assertTrue(expected.message, expected.message.orEmpty().contains("can't be read"))
                }
            }
            core.close()
        } finally {
            dir.deleteRecursively()
        }
    }
}
