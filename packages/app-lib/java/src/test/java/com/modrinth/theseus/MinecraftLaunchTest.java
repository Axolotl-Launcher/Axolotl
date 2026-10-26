package com.modrinth.theseus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;

final class MinecraftLaunchTest {
    private static final String PREFIX = "__THESEUS_UTF8__:";

    @Test
    void decodesUtf8GameArgument() {
        final String original = "E:\\Games\\Minecraft\\profiles\\Prominence\u2122 II";
        final String encoded = PREFIX + Base64.getEncoder().encodeToString(original.getBytes(StandardCharsets.UTF_8));

        assertEquals(original, MinecraftLaunch.decodeGameArgument(encoded));
    }

    @Test
    void leavesAsciiGameArgumentUnchanged() {
        assertEquals("--username", MinecraftLaunch.decodeGameArgument("--username"));
    }

    @Test
    void restoresEscapedPrefix() {
        final String original = PREFIX + "literal";
        final String encoded = PREFIX + Base64.getEncoder().encodeToString(original.getBytes(StandardCharsets.UTF_8));

        assertEquals(original, MinecraftLaunch.decodeGameArgument(encoded));
    }

    @Test
    void rebuildsCommandPropertyWithDecodedGameArguments() {
        final String gameDir = "D:\\MC\\profiles\\神秘启旅 1.10.5 正式版";
        final String encodedGameDir =
                PREFIX + Base64.getEncoder().encodeToString(gameDir.getBytes(StandardCharsets.UTF_8));
        final String[] args = {
            "--username",
            "player",
            "--gameDir",
            MinecraftLaunch.decodeGameArgument(encodedGameDir),
            "--assetsDir",
            "D:\\MC\\meta\\assets"
        };

        final String previousCommand = System.getProperty("sun.java.command");
        try {
            MinecraftLaunch.setGameCommandProperty("net.minecraft.client.main.Main", args);

            assertEquals(
                    "net.minecraft.client.main.Main --username player --gameDir " + gameDir
                            + " --assetsDir D:\\MC\\meta\\assets",
                    System.getProperty("sun.java.command"));

            // Global Packs 19.3.5 extracts its game directory using this pattern.
            final Matcher matcher =
                    Pattern.compile("gameDir\\s(.+?)(?:\\s--|$)").matcher(System.getProperty("sun.java.command"));
            assertTrue(matcher.find());
            // Compared as plain strings: Paths.get() encodes with the JVM's native
            // charset, which cannot represent this name when the locale is unset.
            assertEquals(gameDir, matcher.group(1));
        } finally {
            if (previousCommand == null) {
                System.clearProperty("sun.java.command");
            } else {
                System.setProperty("sun.java.command", previousCommand);
            }
        }
    }
}
