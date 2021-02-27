import React from 'react';
import { Box, Container, Link } from '@chakra-ui/react';
import { Link as RouterLink, Route, Switch } from 'react-router-dom';
import Dashboard from './pages/Dashboard';

function App() {
    return (
        <div className="App">
            <Box
                as="nav"
                height="3.5rem"
                borderBottomWidth="2px"
                borderBottomColor="blue.500"
                display="flex"
                alignItems="center"
            >
                <Container maxW="container.lg">
                    <Link as={RouterLink} to="/" fontSize="xl" _hover={{ textDecoration: 'none' }}>
                        DTVault
                    </Link>
                </Container>
            </Box>
            <Switch>
                <Route path="/" exact component={Dashboard} />
            </Switch>
        </div>
    );
}

export default App;
