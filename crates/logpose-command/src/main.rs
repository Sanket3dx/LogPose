use clap::{Parser, Subcommand};
use logpose_core::{Role, RegistryStore, Service, ServiceInstance, Identity, Protocol, Runtime};
use logpose_db::DbRegistry;
use std::net::SocketAddr;

#[derive(Parser)]
#[command(name = "logpose")]
#[command(about = "LogPose Local Administrative CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, env = "DATABASE_URL", default_value = "logpose.db")]
    db: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Service management
    Service {
        #[command(subcommand)]
        sub: ServiceCommands,
    },
    /// Instance management
    Instance {
        #[command(subcommand)]
        sub: InstanceCommands,
    },
    /// Identity and RBAC management
    Identity {
        #[command(subcommand)]
        sub: IdentityCommands,
    },
    /// Show registry status overview
    Status,
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Register a new service
    Register {
        #[arg(long)]
        name: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        description: String,
    },
    /// List all registered services
    List,
}

#[derive(Subcommand)]
enum InstanceCommands {
    /// Add an instance to a service
    Add {
        #[arg(long)]
        service: String,
        #[arg(long)]
        address: SocketAddr,
        #[arg(long, default_value = "Http")]
        protocol: String,
        #[arg(long, default_value = "Container")]
        runtime: String,
    },
    /// List instances for a service or all instances
    List {
        #[arg(long)]
        service: Option<String>,
    },
}

#[derive(Subcommand)]
enum IdentityCommands {
    /// Add a new identity
    Add {
        #[arg(long)]
        common_name: String,
        #[arg(long)]
        organization: Option<String>,
    },
    /// Assign a role to an identity
    AssignRole {
        #[arg(long)]
        common_name: String,
        #[arg(long)]
        role: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    
    let db = DbRegistry::new(&cli.db)?;
    let registry: &dyn RegistryStore = &db;

    match cli.command {
        Commands::Service { sub } => match sub {
            ServiceCommands::Register { name, code, description } => {
                let service = Service::new(name, code.clone(), description);
                registry.add_service(&service)?;
                println!("Service registered successfully: {}", code);
            }
            ServiceCommands::List => {
                let services = registry.get_all_services()?;
                println!("Registered Services:");
                println!("{:<20} {:<20} {:<30}", "Code", "Name", "Description");
                println!("{}", "-".repeat(70));
                for svc in services {
                    println!("{:<20} {:<20} {:<30}", svc.code, svc.name, svc.description);
                }
            }
        },
        Commands::Instance { sub } => match sub {
            InstanceCommands::Add { service, address, protocol, runtime } => {
                let protocol = match protocol.as_str() {
                    "Http" => Protocol::Http,
                    "Https" => Protocol::Https,
                    "Tcp" => Protocol::Tcp,
                    "Grpc" => Protocol::Grpc,
                    "Udp" => Protocol::Udp,
                    other => Protocol::Custom(other.to_string()),
                };
                let runtime = match runtime.as_str() {
                    "Vm" => Runtime::Vm { provider: None, id: None },
                    "Container" => Runtime::Container { container_id: "".to_string() },
                    "Serverless" => Runtime::Serverless { function_name: "".to_string(), region: None },
                    other => Runtime::Custom(other.to_string()),
                };

                let instance = ServiceInstance::new(
                    service.clone(),
                    address,
                    protocol,
                    runtime,
                    logpose_core::time::now()
                );

                registry.add_instance(&instance)?;
                println!("Instance added to service: {}", service);
            }
            InstanceCommands::List { service } => {
                let instances = if let Some(code) = service {
                    registry.get_instances(&code)?
                } else {
                    registry.get_all_instances()?
                };

                println!("Service Instances:");
                println!("{:<20} {:<20} {:<10} {:<15}", "Service", "Address", "Health", "ID");
                println!("{}", "-".repeat(70));
                for inst in instances {
                    println!("{:<20} {:<20} {:<10} {:<15}", 
                        inst.service_name, 
                        inst.address, 
                        format!("{:?}", inst.health),
                        inst.id
                    );
                }
            }
        },
        Commands::Identity { sub } => match sub {
            IdentityCommands::Add { common_name, organization } => {
                let identity = Identity {
                    common_name: common_name.clone(),
                    organization,
                    roles: vec![Role::Viewer],
                };
                registry.add_identity(&identity)?;
                println!("Identity registered: {}", common_name);
            }
            IdentityCommands::AssignRole { common_name, role } => {
                let role_enum = match role.as_str() {
                    "Admin" | "admin" => Role::Admin,
                    "Agent" | "agent" => Role::Agent,
                    "Viewer" | "viewer" => Role::Viewer,
                    _ => return Err("Invalid role. Use Admin, Agent, or Viewer.".into()),
                };
                registry.add_role_to_identity(&common_name, role_enum.clone())?;
                println!("Role {:?} assigned to identity: {}", role_enum, common_name);
            }
        },
        Commands::Status => {
            let services = registry.get_all_services()?;
            let instances = registry.get_all_instances()?;
            
            println!("LogPose Registry Status Overview");
            println!("{}", "=".repeat(35));
            println!("Total Services:  {}", services.len());
            println!("Total Instances: {}", instances.len());
            
            let healthy = instances.iter().filter(|i| i.health == logpose_core::HealthStatus::Healthy).count();
            println!("Healthy:         {}", healthy);
            println!("Unhealthy:       {}", instances.len() - healthy);
        }
    }

    Ok(())
}
