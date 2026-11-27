/**
 * 朱聆晨
 * [YLZHXT-25] 功能管理脚本添加
 */
if(notExist("SELECT * from auth_function WHERE rec_id = '6576966777095120476'")) {
    executeMultiCommand("""

    INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120476, 'knowledge:meetingVenue', '会议定点场所管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:20:46', '1', NULL, '2026-03-30 20:20:46', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120471, 'knowledge:agencyArea', '国际区划管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:20:22', '1', NULL, '2026-03-30 20:20:22', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120758, 'knowledge:calendar', '日历信息管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:31:23', '1', NULL, '2026-03-30 20:31:23', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120725, 'knowledge:airport', '机场信息管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:25:48', '1', NULL, '2026-03-30 20:25:48', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120747, 'knowledge:busStation', '汽车站点信息管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:30:44', '1', NULL, '2026-03-30 20:30:44', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095120730, 'knowledge:trainStation', '火车站点信息管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 20:26:36', '1', NULL, '2026-03-30 20:26:36', 1, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO auth_function
(rec_id, code, name, parent_id, enabled, url, internal, category, leaf, description, version, rec_created_by, rec_created_org, rec_created_time, rec_modified_by, rec_modified_org, rec_modified_time, rec_version, config, display_sort, mobile_url, icon, all_check, icon_url)
VALUES(6576966777095119978, 'knowledge:branches', '银行支行管理', 6559755812348593074, 1, NULL, NULL, 0, 1, NULL, NULL, '1', NULL, '2026-03-30 19:22:01', '1', NULL, '2026-03-30 19:22:01', 1, NULL, NULL, NULL, NULL, NULL, NULL);
    
	""")
} else {
    println("record with rec_id 6576966777095120476 already exists in table auth_function")
}