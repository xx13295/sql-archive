package cn.bosicloud.sqlarchive;

import com.bosssoft.nontax3.saas.sql.translate.domain.response.BeforeTransformResponse;
import com.bosssoft.nontax3.saas.sql.translate.domain.response.Response;
import com.bosssoft.nontax3.saas.sql.translate.domain.response.SQLResponse;
import com.bosssoft.nontax3.saas.sql.translate.factory.SqlTransformFactory;
import com.bosssoft.nontax3.saas.sql.translate.transform.BaseTransform;
import com.bosssoft.nontax3.saas.sql.translate.transform.oracle.Oracle2OracleTransform;
import com.bosssoft.nontax3.saas.sql.translate.transform.tdsql.Tdsql2TdsqlTransform;

/** 对旧版 sql-translate 引擎的薄封装。 */
final class EngineFacade {
    private final String checkRule;
    private String sharding;

    EngineFacade(String checkRule, String sharding) {
        this.checkRule = checkRule;
        this.sharding = sharding;
    }

    String sharding() {
        return sharding;
    }

    boolean check(String script, String fileName, String sourceDb) {
        BaseTransform transform;
        if ("TDSQL".equalsIgnoreCase(sourceDb)) {
            transform = new Tdsql2TdsqlTransform();
        } else if ("ORACLE".equalsIgnoreCase(sourceDb)) {
            transform = new Oracle2OracleTransform();
        } else {
            throw new IllegalArgumentException("未知的源数据库类型：" + sourceDb);
        }
        Object result = transform.beforeTransFrom(script, sharding, checkRule, fileName);
        if (result instanceof BeforeTransformResponse) {
            return ((BeforeTransformResponse) result).isNormalTableWithTenantCode();
        }
        return false;
    }

    TranslateResult translate(String script, String sourceDb, String targetDb) {
        SqlTransformFactory factory = new SqlTransformFactory();
        Response<Object> response = factory.transFrom(script, sharding, checkRule, sourceDb, targetDb);
        String code = response.getCode();
        Object data = response.getData();
        if (!"200".equals(code) || !(data instanceof SQLResponse)) {
            throw new IllegalStateException(
                data == null ? "引擎返回失败（code=" + code + "）" : String.valueOf(data));
        }
        SQLResponse sqlResponse = (SQLResponse) data;
        String targetSql = sqlResponse.getTargetSql();
        if (targetSql == null || targetSql.trim().isEmpty()) {
            throw new IllegalStateException("引擎未返回目标脚本内容。");
        }
        String nextSharding = sqlResponse.getShardingYml();
        if (nextSharding != null && !nextSharding.trim().isEmpty()) {
            sharding = nextSharding;
        }
        boolean sub = sqlResponse.getSubAndNotCreate() != null && sqlResponse.getSubAndNotCreate();
        return new TranslateResult(targetSql, sub);
    }

    static final class TranslateResult {
        final String sql;
        final boolean subAndNotCreate;

        TranslateResult(String sql, boolean subAndNotCreate) {
            this.sql = sql;
            this.subAndNotCreate = subAndNotCreate;
        }
    }
}