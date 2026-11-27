package cn.bosicloud.sqlarchive;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;

/** 与 Rust 侧交互的行协议工具。 */
final class Protocol {
    private Protocol() {}

    static List<String> readLines(Path path) throws IOException {
        if (!Files.exists(path)) {
            return new ArrayList<String>();
        }
        String text = new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
        List<String> lines = new ArrayList<String>();
        for (String line : text.split("\\r?\\n")) {
            if (!line.isEmpty()) {
                lines.add(line);
            }
        }
        return lines;
    }

    static void writeLines(Path path, List<String> lines) throws IOException {
        StringBuilder builder = new StringBuilder();
        for (String line : lines) {
            builder.append(line).append('\n');
        }
        Files.write(path, builder.toString().getBytes(StandardCharsets.UTF_8));
    }

    static String readText(Path path) throws IOException {
        return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
    }

    static void writeText(Path path, String text) throws IOException {
        Path parent = path.getParent();
        if (parent != null) {
            Files.createDirectories(parent);
        }
        Files.write(path, text.getBytes(StandardCharsets.UTF_8));
    }

    static String encode(String value) {
        if (value == null) {
            return "";
        }
        return Base64.getEncoder().encodeToString(value.getBytes(StandardCharsets.UTF_8));
    }

    static String decode(String value) {
        if (value == null || value.isEmpty()) {
            return "";
        }
        return new String(Base64.getDecoder().decode(value), StandardCharsets.UTF_8);
    }

    static Path path(String value) {
        return Paths.get(value);
    }

    static String firstLine(Throwable error) {
        String message = error.getMessage();
        if (message == null || message.trim().isEmpty()) {
            message = error.getClass().getSimpleName();
        }
        int newline = message.indexOf('\n');
        return newline >= 0 ? message.substring(0, newline) : message;
    }
}