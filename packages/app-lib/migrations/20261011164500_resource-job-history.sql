CREATE TRIGGER resource_install_job_delete
BEFORE DELETE ON install_jobs
BEGIN
    UPDATE resource_operation_plans SET job_id = NULL WHERE job_id = OLD.id;
END;
