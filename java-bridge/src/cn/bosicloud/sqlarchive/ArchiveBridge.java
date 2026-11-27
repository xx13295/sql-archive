package cn.bosicloud.sqlarchive;

import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * sql-archive 的 Java 桥接入口。
 *
 * <p>用法：{@code ArchiveBridge <runDir> <resultFile>}
 *
 * <p>runDir 必须是纯 ASCII 路径。输入约定：
 * <ul>
 *   <li>request.tsv：CHECK/TRANSLATE 任务行</li>
 *   <li>scripts/&lt;index&gt;.groovy：脚本内容（UTF-8）</li>
 *   <li>names/&lt;index&gt;.txt：原始文件名（UTF-8）</li>
 *   <li>check-rule.yml、sharding.in</li>
 * </ul>
 * 输出：results.tsv、out/&lt;index&gt;__&lt;target&gt;.sql、sharding.out、log.txt。
 */
public final class ArchiveBridge {
    private ArchiveBridge() {}

    public static void main(String[] args) {
        if (args.length < 2) {
            System.err.println("usage: ArchiveBridge <runDir> <resultFile>");
            System.exit(2);
        }
        Path runDir = Protocol.path(args[0]);
        Path resultFile = Protocol.path(args[1]);
        List<String> results = new ArrayList<String>();
        List<String> log = new ArrayList<String>();
        try {
            List<String> requests = Protocol.readLines(runDir.resolve("request.tsv"));
            String checkRulePath = runDir.resolve("check-rule.yml").toString();
            String checkRule = Files.exists(runDir.resolve("check-rule.yml"))
                ? Protocol.readText(runDir.resolve("check-rule.yml"))
                : "";
            String sharding = Files.exists(runDir.resolve("sharding.in"))
                ? Protocol.readText(runDir.resolve("sharding.in"))
                : "spring:";
            if (checkRule.trim().isEmpty()) {
                throw new IllegalStateException("check-rule.yml 为空或不存在");
            }
            EngineFacade engine = new EngineFacade(checkRule, sharding);
            for (String request : requests) {
                String[] fields = request.split("\t", -1);
                if (fields.length < 3) {
                    log.add("malformed request: " + request);
                    continue;
                }
                String kind = fields[0];
                String index = fields[1];
                String sourceDb = fields[2];
                try {
                    Path scriptPath = runDir.resolve("scripts").resolve(index + ".groovy");
                    String script = Protocol.readText(scriptPath);
                    if ("CHECK".equals(kind)) {
                        String name = Protocol.readText(runDir.resolve("names").resolve(index + ".txt"));
                        boolean tenant = engine.check(script, name, sourceDb);
                        results.add("OK\t" + index + "\tCHECK\t" + (tenant ? "1" : "0") + "\t");
                    } else if ("TRANSLATE".equals(kind) && fields.length >= 4) {
                        String targetDb = fields[3];
                        EngineFacade.TranslateResult translated = engine.translate(script, sourceDb, targetDb);
                        Path output = runDir
                            .resolve("out")
                            .resolve(index + "__" + targetDb + ".sql");
                        Protocol.writeText(output, translated.sql);
                        results.add(
                            "OK\t" + index + "\tTRANSLATE\t"
                                + (translated.subAndNotCreate ? "1" : "0") + "\t\t" + targetDb);
                    } else {
                        throw new IllegalArgumentException("未知请求：" + request);
                    }
                } catch (Throwable error) {
                    String targetField = fields.length >= 4 ? fields[3] : "";
                    results.add(
                        "ERR\t" + index + "\t" + kind + "\t0\t"
                            + Protocol.encode(Protocol.firstLine(error)) + "\t" + targetField);
                    log.add("request failed: " + request);
                    log.add(stackTrace(error));
                }
            }
            Protocol.writeText(runDir.resolve("sharding.out"), engine.sharding());
            results.add("DONE\t0\tEND\t0\t");
        } catch (Throwable error) {
            results.add("FATAL\t0\tEND\t0\t" + Protocol.encode(Protocol.firstLine(error)));
            log.add(stackTrace(error));
        }
        try {
            Protocol.writeLines(resultFile, results);
            Protocol.writeLines(runDir.resolve("log.txt"), log);
        } catch (Throwable error) {
            error.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }

    private static String stackTrace(Throwable error) {
        StringWriter writer = new StringWriter();
        error.printStackTrace(new PrintWriter(writer));
        return writer.toString();
    }
}