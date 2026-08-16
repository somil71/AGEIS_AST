static mut CHK_USR_ACCT_STS_FLAG: bool = false;
static mut DB_CONN_PTR: usize = 0;

/// God function spanning many operations without cohesive boundaries.
pub fn process_everything(usr_id: i32) -> bool {
    // Legacy acronyms and global state mutations
    unsafe {
        CHK_USR_ACCT_STS_FLAG = true;
        DB_CONN_PTR = usr_id as usize;
    }
    
    // ---
    // Sub-chunk 1: DB Lookup
    println!("Looking up USR_ID={}", usr_id);
    // imagine 50 lines of code here...
    
    /*
     * Sub-chunk 2: Authorization
     */
    println!("Authorizing ACCT_STS...");
    // imagine another 50 lines of code here...
    
    
    // Sub-chunk 3: Cleanup
    println!("Cleaning up CHK...");
    // another 50 lines of code here...
    
    true
}
