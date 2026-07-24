/*
* SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
* SPDX-License-Identifier: Apache-2.0
*/
use super::Artifact;
use super::Schedule;

impl Artifact for Schedule {
    fn get_name(&self) -> String {
        self.metadata.name.clone()
    }
}

impl Schedule {
    pub fn get_spec(&self) -> &Option<Vec<ScheduleSpec>> {
        &self.spec
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ScheduleSpec {
    pub name: String,
    pub priority: i32,
    pub policy: SchedPolicy,
    pub cpu_affinity: u64,
    pub period: i32,
    pub release_time: i32,
    pub runtime: i32,
    pub deadline: i32,
    pub node_id: String,
    pub max_dmiss: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SchedPolicy {
    // SCHED_NORMAL
    NORMAL = 0,
    // SCHED_FIFO
    FIFO = 1,
    // SCHED_RR
    RR = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_spec_properties() {
        let spec = ScheduleSpec {
            name: "sched1".to_string(),
            priority: 10,
            policy: SchedPolicy::FIFO,
            cpu_affinity: 3,
            period: 1000,
            release_time: 0,
            runtime: 500,
            deadline: 1000,
            node_id: "HPC".to_string(),
            max_dmiss: 1,
        };

        assert_eq!(spec.name, "sched1");
        assert_eq!(spec.priority, 10);
        assert_eq!(spec.policy, SchedPolicy::FIFO);
        assert_eq!(spec.cpu_affinity, 3);
        assert_eq!(spec.max_dmiss, 1);
    }
}
